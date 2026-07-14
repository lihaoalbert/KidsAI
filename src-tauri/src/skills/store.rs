// W10 — SkillsStore: TrustedStorage 包装, 管理 app_data_dir/skills/{id}/
//
// 文件布局:
//   skills/
//     installed.json        ← { id: { version, enabled, installed_at, audience } }
//     {skill_id}/
//       manifest.json       ← SkillManifestFull (含 publisher_signature)
//       assets/             ← 图片/音频
//       prompts/            ← yaml 片段
//       templates/          ← 角色/分镜模板
//
// 不解密 prompts — skill 是公开包, 不走 license_token; server 直接下原文.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::marketplace_client::{MarketplaceClient, MarketplaceError};
use crate::skills::{Audience, InstallReceipt, SkillManifestFull, SkillSummary};
use crate::trusted_storage::{StorageError, TrustedStorage};

#[derive(Debug, thiserror::Error)]
pub enum SkillsStoreError {
    #[error("storage: {0}")]
    Storage(#[from] StorageError),
    #[error("marketplace: {0}")]
    Marketplace(#[from] MarketplaceError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse json: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("skill {0} not found")]
    NotFound(String),
    #[error("manifest verification failed: {0}")]
    Verify(String),
    #[error("capacity exceeded: max {0}")]
    Capacity(usize),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InstalledIndex {
    /// skill_id → record
    pub skills: HashMap<String, InstalledRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledRecord {
    pub version: String,
    pub enabled: bool,
    pub installed_at: i64,
    pub audience: Audience,
}

const INSTALLED_INDEX: &str = "installed.json";
const MAX_INSTALLED_SKILLS: usize = 10;

pub struct SkillsStore {
    storage: TrustedStorage,
}

impl SkillsStore {
    pub fn new(storage: TrustedStorage) -> Self {
        Self { storage }
    }

    pub fn storage(&self) -> &TrustedStorage {
        &self.storage
    }

    pub fn list_installed(&self) -> Result<Vec<SkillSummary>, SkillsStoreError> {
        let idx = self.read_index()?;
        let mut out: Vec<SkillSummary> = idx
            .skills
            .into_iter()
            .map(|(id, r)| SkillSummary {
                id,
                name: String::new(), // 名字查 manifest
                version: r.version,
                enabled: r.enabled,
                installed_at: r.installed_at,
                audience: r.audience,
            })
            .collect();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }

    pub fn is_enabled(&self, skill_id: &str) -> Result<bool, SkillsStoreError> {
        let idx = self.read_index()?;
        Ok(idx.skills.get(skill_id).map(|r| r.enabled).unwrap_or(false))
    }

    pub fn set_enabled(&self, skill_id: &str, enabled: bool) -> Result<(), SkillsStoreError> {
        let mut idx = self.read_index()?;
        let r = idx
            .skills
            .get_mut(skill_id)
            .ok_or_else(|| SkillsStoreError::NotFound(skill_id.to_string()))?;
        r.enabled = enabled;
        self.write_index(&idx)
    }

    pub fn uninstall(&self, skill_id: &str) -> Result<(), SkillsStoreError> {
        let mut idx = self.read_index()?;
        idx.skills.remove(skill_id);
        self.write_index(&idx)?;
        let rel = std::path::Path::new(skill_id);
        self.storage.remove_dir_all(rel)?;
        Ok(())
    }

    /// 端到端 install: 拉 manifest → 验签 → 拉每个 asset → 写盘 → 登记 installed.json.
    pub async fn download_and_install(
        &self,
        skill_id: &str,
        client: &MarketplaceClient,
    ) -> Result<InstallReceipt, SkillsStoreError> {
        // 容量预检
        let idx = self.read_index()?;
        if !idx.skills.contains_key(skill_id) && idx.skills.len() >= MAX_INSTALLED_SKILLS {
            return Err(SkillsStoreError::Capacity(MAX_INSTALLED_SKILLS));
        }

        // 1. 拉 manifest
        let manifest_path = format!("/api/v1/skills/{}/manifest", skill_id);
        let (bytes, _meta) = client.get_bytes(&manifest_path).await?;
        let manifest: SkillManifestFull = serde_json::from_slice(&bytes)?;
        if manifest.id != skill_id {
            return Err(SkillsStoreError::Verify(format!(
                "manifest id {} ≠ requested {}",
                manifest.id, skill_id
            )));
        }

        // 2. 验签 (走 LicenseSigner 单例)
        let canonical = manifest_canonical(&manifest);
        match crate::license_signer::LicenseSigner::get() {
            Some(signer) => {
                signer
                    .verify(&canonical, &manifest.publisher_signature, &manifest.publisher_pubkey_id)
                    .map_err(|e| SkillsStoreError::Verify(format!("signature: {e}")))?;
            }
            None => {
                return Err(SkillsStoreError::Verify(
                    "LicenseSigner not initialized".into(),
                ));
            }
        }

        // 3. 写 manifest 到本地
        self.storage
            .write_atomic(std::path::Path::new(&format!("{skill_id}/manifest.json")), &bytes)?;

        // 4. 拉每个 asset → sha256 校验 → 写盘
        for asset in &manifest.assets {
            let url = format!("/api/v1/skills/{}/blob?file={}", skill_id, asset.path);
            let (data, _meta) = client.get_bytes(&url).await?;
            let actual_sha = crate::trusted_storage::hex_sha256(&data);
            if actual_sha != asset.sha256 {
                return Err(SkillsStoreError::Verify(format!(
                    "asset {} sha mismatch ({} ≠ {})",
                    asset.path, actual_sha, asset.sha256
                )));
            }
            let rel_str = format!("{skill_id}/{}", asset.path);
            let rel = std::path::Path::new(&rel_str);
            self.storage.write_atomic(rel, &data)?;
        }

        // 5. 拉每个 prompt yaml (共用 blob endpoint)
        for prompt in &manifest.prompts {
            let url = format!("/api/v1/skills/{}/blob?file={}", skill_id, prompt.file);
            let (data, _meta) = client.get_bytes(&url).await?;
            let actual_sha = crate::trusted_storage::hex_sha256(&data);
            if actual_sha != prompt.sha256 {
                return Err(SkillsStoreError::Verify(format!(
                    "prompt {} sha mismatch", prompt.file
                )));
            }
            let rel_str = format!("{skill_id}/{}", prompt.file);
            let rel = std::path::Path::new(&rel_str);
            self.storage.write_atomic(rel, &data)?;
        }

        // 6. 登记 installed.json
        let now = now_millis();
        let mut idx = self.read_index().unwrap_or_default();
        idx.skills.insert(
            skill_id.to_string(),
            InstalledRecord {
                version: manifest.version.clone(),
                enabled: true,
                installed_at: now,
                audience: manifest.audience.clone(),
            },
        );
        self.write_index(&idx)?;

        Ok(InstallReceipt {
            skill_id: skill_id.to_string(),
            version: manifest.version,
            size_bytes: manifest.size_bytes,
            installed_at: now,
            audit_id: uuid::Uuid::new_v4().to_string(),
        })
    }

    fn read_index(&self) -> Result<InstalledIndex, SkillsStoreError> {
        match self
            .storage
            .read_bytes(std::path::Path::new(INSTALLED_INDEX))?
        {
            Some(b) => Ok(serde_json::from_slice(&b)?),
            None => Ok(InstalledIndex::default()),
        }
    }

    fn write_index(&self, idx: &InstalledIndex) -> Result<(), SkillsStoreError> {
        let b = serde_json::to_vec_pretty(idx)?;
        self.storage
            .write_atomic(std::path::Path::new(INSTALLED_INDEX), &b)?;
        Ok(())
    }
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// 构造 manifest canonical: 移除 publisher_signature 字段后 JSON 序列化.
/// 与 server 端 sign_canonical 保持一致: server 端用 `json.dumps(sort_keys=True, separators=(",", ":"))`,
/// 这里 to_value 再移除 signature 后 to_string 即可, serde_json 默认按 key 插入顺序输出 (而非字典序).
/// 关键约定: server 端 sign 的 canonical 必须等于 client 端 verify 的 canonical.
/// 当前我们双方都用 serde_json to_string(value), 字段顺序依赖 serde derive 顺序. 为避免漂移,
/// server 端也应改用「序列化后再 strip signature」的方式 — 见 secrets_publisher.sign_canonical 注释.
/// 本函数: 先 to_value, remove publisher_signature, 再 to_string — 字段顺序不变.
pub fn manifest_canonical(m: &SkillManifestFull) -> String {
    let mut v = serde_json::to_value(m).expect("serialize manifest");
    if let Some(obj) = v.as_object_mut() {
        obj.remove("publisher_signature");
    }
    serde_json::to_string(&v).expect("re-serialize canonical")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::SkillTemplates;
    use std::path::Path;
    use tempfile::tempdir;

    fn fixture(dir: &Path) -> TrustedStorage {
        TrustedStorage::new(dir)
    }

    #[test]
    fn empty_store_lists_nothing() {
        let dir = tempdir().unwrap();
        let store = SkillsStore::new(fixture(dir.path()));
        assert!(store.list_installed().unwrap().is_empty());
    }

    #[test]
    fn set_enabled_on_missing_skill_errors() {
        let dir = tempdir().unwrap();
        let store = SkillsStore::new(fixture(dir.path()));
        let r = store.set_enabled("nonexistent", true);
        assert!(matches!(r, Err(SkillsStoreError::NotFound(_))));
    }

    #[test]
    fn uninstall_missing_is_ok() {
        let dir = tempdir().unwrap();
        let store = SkillsStore::new(fixture(dir.path()));
        store.uninstall("nope").unwrap(); // 不存在也应幂等
    }

    #[test]
    fn write_then_read_index_roundtrip() {
        let dir = tempdir().unwrap();
        let store = SkillsStore::new(fixture(dir.path()));
        let mut idx = InstalledIndex::default();
        idx.skills.insert(
            "eng-adventure".into(),
            InstalledRecord {
                version: "v1.a3f9c2".into(),
                enabled: true,
                installed_at: 1234567890,
                audience: Audience::Child,
            },
        );
        store.write_index(&idx).unwrap();
        let r = store.read_index().unwrap();
        assert_eq!(r.skills.len(), 1);
        let rec = r.skills.get("eng-adventure").unwrap();
        assert_eq!(rec.version, "v1.a3f9c2");
        assert_eq!(rec.audience, Audience::Child);
    }

    #[test]
    fn manifest_canonical_omits_signature_field() {
        let m = SkillManifestFull {
            schema: "kidsai.skill/1".into(),
            id: "x".into(),
            name: "X".into(),
            version: "v1".into(),
            publisher: "kidsai-official".into(),
            min_app_version: "0.4.0".into(),
            age_tier: vec![1, 2],
            category: "language".into(),
            audience: Audience::Child,
            assets: vec![],
            prompts: vec![],
            templates: SkillTemplates {
                characters: vec![],
                story_arcs: vec![],
            },
            extends: SkillExtends_dummy(),
            credits_per_use: 0,
            daily_quota: 0,
            homepage: None,
            size_bytes: 0,
            publisher_signature: "BASE64SIGNATURE".into(),
            publisher_pubkey_id: "kidsai-dev-2026-q3".into(),
        };
        let c = manifest_canonical(&m);
        assert!(!c.contains("publisher_signature"));
        assert!(!c.contains("BASE64SIGNATURE"));
        assert!(c.contains("kidsai.skill/1"));
    }

    fn SkillExtends_dummy() -> crate::skills::SkillExtends {
        crate::skills::SkillExtends {
            tabs: vec![],
            tools: vec![],
            characters_inject_into: None,
        }
    }
}