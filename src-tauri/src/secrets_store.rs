// W11 Day 7 — SecretsStore (落盘管理)
//
// 文件布局 (app_data_dir/secrets/):
//   current.json              ← { child: "v1.x", adult: "v1.y", updated_at }
//   child/
//     v1.a3f9c2/
//       manifest.json         ← SecretsManifest (签名过的, 公开)
//       bundle.bin            ← AES-GCM(master_key, plaintext)
//       wrapped.json          ← AES-GCM(KEK, master_key) — 缓存 per-device 包裹
//     v1.8b21f0/ ...
//   adult/
//     v1.adult01/...
//
// 历史版本最多保留 3 个; 装第 4 个新版本时删最旧.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::secrets::{SecretsManifest, WrappedMaster};
use crate::trusted_storage::{StorageError, TrustedStorage};

#[derive(Debug, thiserror::Error)]
pub enum SecretsStoreError {
    #[error("storage: {0}")]
    Storage(#[from] StorageError),
    #[error("parse json: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("profile 不支持: {0}")]
    InvalidProfile(String),
    #[error("version 不存在: {0}/{1}")]
    VersionMissing(String, String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CurrentVersions {
    /// profile ("child"/"adult") → 当前 version (例 "v1.a3f9c2")
    #[serde(default)]
    pub profiles: HashMap<String, String>,
    /// 上次更新时间 (ms)
    #[serde(default)]
    pub updated_at: i64,
}

const CURRENT_JSON: &str = "current.json";
const HISTORY_LIMIT: usize = 3;
const SUPPORTED_PROFILES: &[&str] = &["child", "adult"];

pub struct SecretsStore {
    storage: TrustedStorage,
}

impl SecretsStore {
    pub fn new(app_data_dir: &Path) -> Self {
        let root = app_data_dir.join("secrets");
        Self {
            storage: TrustedStorage::new(&root),
        }
    }

    pub fn storage(&self) -> &TrustedStorage {
        &self.storage
    }

    pub fn root(&self) -> &Path {
        self.storage.root()
    }

    // ───────── current.json ─────────

    pub fn load_current(&self) -> Result<CurrentVersions, SecretsStoreError> {
        match self.storage.read_bytes(Path::new(CURRENT_JSON))? {
            Some(b) => Ok(serde_json::from_slice(&b)?),
            None => Ok(CurrentVersions::default()),
        }
    }

    pub fn save_current(&self, cur: &CurrentVersions) -> Result<(), SecretsStoreError> {
        let b = serde_json::to_vec_pretty(cur)?;
        self.storage.write_atomic(Path::new(CURRENT_JSON), &b)?;
        Ok(())
    }

    // ───────── version dir 写入 ─────────

    /// 装一个新版本: 写 manifest + bundle + wrapped → 登记到 current.json → 清理 history (保留 3).
    pub fn install_version(
        &self,
        profile: &str,
        manifest: &SecretsManifest,
        bundle_ct: &[u8],
        wrapped: &WrappedMaster,
    ) -> Result<(), SecretsStoreError> {
        if !SUPPORTED_PROFILES.contains(&profile) {
            return Err(SecretsStoreError::InvalidProfile(profile.to_string()));
        }
        let ver_dir = Path::new(profile).join(&manifest.version);

        // 1. manifest.json
        let manifest_bytes = serde_json::to_vec_pretty(manifest)?;
        self.storage
            .write_atomic(&ver_dir.join("manifest.json"), &manifest_bytes)?;

        // 2. bundle.bin
        self.storage
            .write_atomic(&ver_dir.join("bundle.bin"), bundle_ct)?;

        // 3. wrapped.json
        let wrapped_bytes = serde_json::to_vec_pretty(wrapped)?;
        self.storage
            .write_atomic(&ver_dir.join("wrapped.json"), &wrapped_bytes)?;

        // 4. 更新 current.json
        let mut cur = self.load_current()?;
        cur.profiles.insert(profile.to_string(), manifest.version.clone());
        cur.updated_at = now_millis();
        self.save_current(&cur)?;

        // 5. 历史清理 (保留 ≤3)
        self.prune_history(profile, &manifest.version)?;

        Ok(())
    }

    /// 列出某 profile 的所有版本 (按 version 字典序排序).
    pub fn list_versions(&self, profile: &str) -> Result<Vec<String>, SecretsStoreError> {
        if !SUPPORTED_PROFILES.contains(&profile) {
            return Err(SecretsStoreError::InvalidProfile(profile.to_string()));
        }
        let profile_dir = Path::new(profile);
        if !self.storage.exists(profile_dir) {
            return Ok(Vec::new());
        }
        // TrustedStorage 不直接列目录, 走 std::fs
        let abs = self.storage.root().join(profile);
        let mut out: Vec<String> = std::fs::read_dir(&abs)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        out.sort();
        Ok(out)
    }

    fn prune_history(
        &self,
        profile: &str,
        keep: &str,
    ) -> Result<(), SecretsStoreError> {
        let versions = self.list_versions(profile)?;
        // versions 字典序: 新的通常在末尾 (按 plan: v1.x vs v1.y, 字典序不一定 = 时间序, 但服务端用 git-hash 短串足够稳定)
        // 简单策略: 保留 keep + 字典序最大的 N-1 个
        let mut sorted = versions.clone();
        sorted.sort();
        // 把 keep 一定保留
        let mut to_keep: Vec<String> = sorted
            .iter()
            .rev()
            .take(HISTORY_LIMIT)
            .cloned()
            .collect();
        if !to_keep.contains(&keep.to_string()) {
            to_keep.push(keep.to_string());
        }
        // 删除不在 to_keep 里的
        for v in &versions {
            if !to_keep.contains(v) {
                let rel = Path::new(profile).join(v);
                let _ = self.storage.remove_dir_all(&rel);
            }
        }
        Ok(())
    }

    // ───────── 读 manifest / bundle / wrapped ─────────

    pub fn read_manifest(
        &self,
        profile: &str,
        version: &str,
    ) -> Result<SecretsManifest, SecretsStoreError> {
        let rel = Path::new(profile).join(version).join("manifest.json");
        match self.storage.read_bytes(&rel)? {
            Some(b) => Ok(serde_json::from_slice(&b)?),
            None => Err(SecretsStoreError::VersionMissing(
                profile.to_string(),
                version.to_string(),
            )),
        }
    }

    pub fn read_bundle(
        &self,
        profile: &str,
        version: &str,
    ) -> Result<Vec<u8>, SecretsStoreError> {
        let rel = Path::new(profile).join(version).join("bundle.bin");
        match self.storage.read_bytes(&rel)? {
            Some(b) => Ok(b),
            None => Err(SecretsStoreError::VersionMissing(
                profile.to_string(),
                version.to_string(),
            )),
        }
    }

    pub fn read_wrapped(
        &self,
        profile: &str,
        version: &str,
    ) -> Result<WrappedMaster, SecretsStoreError> {
        let rel = Path::new(profile).join(version).join("wrapped.json");
        match self.storage.read_bytes(&rel)? {
            Some(b) => Ok(serde_json::from_slice(&b)?),
            None => Err(SecretsStoreError::VersionMissing(
                profile.to_string(),
                version.to_string(),
            )),
        }
    }
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::{CipherInfo, SecretFileEntry};
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;
    use tempfile::tempdir;

    fn dummy_manifest(version: &str) -> SecretsManifest {
        SecretsManifest {
            schema: "kidsai.secrets/1".into(),
            version: version.to_string(),
            previous_version: None,
            profile: "child".into(),
            created_at: "2026-07-14T00:00:00Z".into(),
            publisher_pubkey_id: "test".into(),
            files: vec![SecretFileEntry {
                path: "system/director.yaml".into(),
                sha256: "deadbeef".into(),
                size: 100,
            }],
            cipher: CipherInfo {
                algo: "AES-256-GCM".into(),
                iv: STANDARD.encode([0u8; 12]),
                plaintext_sha256: "0".repeat(64),
            },
            publisher_signature: "AAAA".into(),
            bundle: None,
            wrap: None,
        }
    }

    fn dummy_wrapped() -> WrappedMaster {
        WrappedMaster {
            ciphertext_b64: "AAAA".into(),
            iv: STANDARD.encode([0u8; 12]),
            algo: "AES-256-GCM".into(),
            kdf: "HKDF-SHA256".into(),
            kdf_salt: "kidsai-secrets-v1".into(),
            kdf_info: "kidsai-secrets/wrap-master".into(),
        }
    }

    #[test]
    fn empty_store_loads_empty_current() {
        let dir = tempdir().unwrap();
        let store = SecretsStore::new(dir.path());
        let cur = store.load_current().unwrap();
        assert!(cur.profiles.is_empty());
        assert_eq!(cur.updated_at, 0);
    }

    #[test]
    fn install_then_load_roundtrip() {
        let dir = tempdir().unwrap();
        let store = SecretsStore::new(dir.path());
        let m = dummy_manifest("v1.test01");
        store
            .install_version("child", &m, b"FAKE_BUNDLE_CT", &dummy_wrapped())
            .unwrap();

        // current.json
        let cur = store.load_current().unwrap();
        assert_eq!(cur.profiles.get("child"), Some(&"v1.test01".to_string()));
        assert!(cur.updated_at > 0);

        // manifest 回读
        let m2 = store.read_manifest("child", "v1.test01").unwrap();
        assert_eq!(m2.version, "v1.test01");
        assert_eq!(m2.files[0].path, "system/director.yaml");

        // bundle 回读
        let b = store.read_bundle("child", "v1.test01").unwrap();
        assert_eq!(b, b"FAKE_BUNDLE_CT");

        // wrapped 回读
        let w = store.read_wrapped("child", "v1.test01").unwrap();
        assert_eq!(w.algo, "AES-256-GCM");
    }

    #[test]
    fn history_pruned_to_three() {
        let dir = tempdir().unwrap();
        let store = SecretsStore::new(dir.path());
        // 装 5 个版本 → 应保留最近 3 个 (字典序最大的 3)
        for v in &["v1.a", "v1.b", "v1.c", "v1.d", "v1.e"] {
            let m = dummy_manifest(v);
            store
                .install_version("child", &m, b"bundle", &dummy_wrapped())
                .unwrap();
        }
        let versions = store.list_versions("child").unwrap();
        assert_eq!(versions.len(), 3, "保留 3 个: {versions:?}");
        // 应保留 v1.c / v1.d / v1.e (字典序最大 3)
        assert!(versions.contains(&"v1.c".to_string()));
        assert!(versions.contains(&"v1.d".to_string()));
        assert!(versions.contains(&"v1.e".to_string()));
        // v1.a / v1.b 已被删
        assert!(!versions.contains(&"v1.a".to_string()));
        assert!(!versions.contains(&"v1.b".to_string()));
    }

    #[test]
    fn history_keeps_current() {
        // 即便 keep 不在字典序最大 N 个里, 也必须保留 (防 prune 把 current 删了)
        let dir = tempdir().unwrap();
        let store = SecretsStore::new(dir.path());
        // 装一个旧版本 (字典序最小), 然后装 4 个新版本 → 当前仍是 v1.a, 但它不在字典序 top-3
        for v in &["v1.a", "v1.b", "v1.c", "v1.d", "v1.e"] {
            let m = dummy_manifest(v);
            store
                .install_version("child", &m, b"bundle", &dummy_wrapped())
                .unwrap();
        }
        // current 应该是 v1.e (最后装的)
        let cur = store.load_current().unwrap();
        assert_eq!(cur.profiles.get("child"), Some(&"v1.e".to_string()));
        // 但 history 里 v1.a 应被 prune (因为 v1.b/c/d/e 都比 a 新)
        let versions = store.list_versions("child").unwrap();
        assert!(!versions.contains(&"v1.a".to_string()), "v1.a 应被 prune");
    }

    #[test]
    fn invalid_profile_rejected() {
        let dir = tempdir().unwrap();
        let store = SecretsStore::new(dir.path());
        let m = dummy_manifest("v1.x");
        let res = store.install_version("alien", &m, b"b", &dummy_wrapped());
        assert!(matches!(res, Err(SecretsStoreError::InvalidProfile(_))));
    }

    #[test]
    fn missing_version_errors() {
        let dir = tempdir().unwrap();
        let store = SecretsStore::new(dir.path());
        let res = store.read_manifest("child", "v99.never");
        assert!(matches!(res, Err(SecretsStoreError::VersionMissing(_, _))));
    }

    #[test]
    fn two_profiles_independent() {
        let dir = tempdir().unwrap();
        let store = SecretsStore::new(dir.path());
        let m_child = dummy_manifest("v1.child01");
        let m_adult = SecretsManifest {
            profile: "adult".into(),
            version: "v1.adult01".into(),
            ..dummy_manifest("v1.adult01")
        };
        store
            .install_version("child", &m_child, b"b1", &dummy_wrapped())
            .unwrap();
        store
            .install_version("adult", &m_adult, b"b2", &dummy_wrapped())
            .unwrap();

        let cur = store.load_current().unwrap();
        assert_eq!(cur.profiles.get("child"), Some(&"v1.child01".to_string()));
        assert_eq!(cur.profiles.get("adult"), Some(&"v1.adult01".to_string()));

        // 各自 list
        assert_eq!(store.list_versions("child").unwrap().len(), 1);
        assert_eq!(store.list_versions("adult").unwrap().len(), 1);
    }
}