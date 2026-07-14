// License 本地持久化 (W4.5 B2)
//
// 把 server 返回的 license + API keys 存到 app_data_dir/license.json (chmod 600).
// 桌面启动时 load, 调用前用 license_token 当 Bearer.
//
// 设计:
// - 文件不存在 → None, 桌面需走 Onboarding 激活
// - load 失败 (损坏 / 反序列化错) → None + eprintln 日志, 让用户重新激活
// - save 走 tmp + rename, 防半写损坏
// - 不存学币余额/历史 (server 是权威)

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const LICENSE_FILENAME: &str = "license.json";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LicenseFile {
    pub device_id: String,
    pub license_token: String,
    pub llm_api_key: String,
    pub video_api_key: String,
    /// 上次 server 返回的余额 (前端显示用, 不是权威)
    #[serde(default)]
    pub last_balance: Option<i64>,
    /// 激活时填的昵称
    #[serde(default)]
    pub nickname: Option<String>,
    /// 激活时填的年级 (0-3)
    #[serde(default)]
    pub age_tier: Option<u8>,
    /// 首次激活时间 (ms)
    #[serde(default)]
    pub activated_at: Option<i64>,
    /// 用户当前模式 (Part C). 默认 Child, 家长 PIN 解锁后切 Adult.
    /// 缺省 → Child (向后兼容老 license.json).
    #[serde(default)]
    pub mode: UserMode,
    /// 上次模式切换时间 (ms)
    #[serde(default)]
    pub mode_switched_at: Option<i64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum UserMode {
    #[default]
    Child,
    Adult,
}

pub struct LicenseStore {
    path: PathBuf,
}

impl LicenseStore {
    pub fn new(app_data_dir: &Path) -> Self {
        Self {
            path: app_data_dir.join(LICENSE_FILENAME),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Option<LicenseFile> {
        match fs::read_to_string(&self.path) {
            Ok(text) => match serde_json::from_str::<LicenseFile>(&text) {
                Ok(f) if !f.device_id.is_empty() && !f.license_token.is_empty() => Some(f),
                Ok(_) => {
                    eprintln!("[license_store] license.json 字段为空, 需重新激活");
                    None
                }
                Err(e) => {
                    eprintln!("[license_store] license.json 解析失败: {e}");
                    None
                }
            },
            Err(_) => None,
        }
    }

    pub fn save(&self, file: &LicenseFile) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("create dir: {e}"))?;
        }
        let json = serde_json::to_string_pretty(file).map_err(|e| format!("serialize: {e}"))?;
        // tmp + rename 防半写
        let tmp = self.path.with_extension("json.tmp");
        {
            let mut f = fs::File::create(&tmp).map_err(|e| format!("create tmp: {e}"))?;
            f.write_all(json.as_bytes())
                .map_err(|e| format!("write: {e}"))?;
            f.sync_all().map_err(|e| format!("sync: {e}"))?;
        }
        // chmod 600 (Unix only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
        }
        fs::rename(&tmp, &self.path).map_err(|e| format!("rename: {e}"))?;
        Ok(())
    }

    pub fn delete(&self) -> Result<(), String> {
        if self.path.exists() {
            fs::remove_file(&self.path).map_err(|e| format!("remove: {e}"))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn save_then_load_roundtrip() {
        let dir = tempdir().unwrap();
        let store = LicenseStore::new(dir.path());
        let f = LicenseFile {
            device_id: "dev-001".to_string(),
            license_token: "eyJ...".to_string(),
            llm_api_key: "sk-llm".to_string(),
            video_api_key: "sk-vid".to_string(),
            last_balance: Some(85),
            nickname: Some("小明".to_string()),
            age_tier: Some(2),
            activated_at: Some(1718234567890),
            ..Default::default()
        };
        store.save(&f).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(loaded.device_id, "dev-001");
        assert_eq!(loaded.license_token, "eyJ...");
        assert_eq!(loaded.last_balance, Some(85));
        assert_eq!(loaded.mode, crate::license_store::UserMode::default());
    }

    #[test]
    fn mode_field_defaults_to_child_when_loading_old_license() {
        let dir = tempdir().unwrap();
        let store = LicenseStore::new(dir.path());
        // 老格式 license.json 不含 mode 字段 — 加载应默认 Child, 不报错.
        let old = serde_json::json!({
            "device_id": "dev-old",
            "license_token": "tok",
            "llm_api_key": "",
            "video_api_key": "",
        });
        std::fs::write(store.path(), old.to_string()).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(loaded.mode, crate::license_store::UserMode::Child);
        assert_eq!(loaded.mode_switched_at, None);
    }

    #[test]
    fn mode_field_roundtrips_as_kebab_case() {
        let dir = tempdir().unwrap();
        let store = LicenseStore::new(dir.path());
        let f = LicenseFile {
            device_id: "d".into(),
            license_token: "t".into(),
            llm_api_key: "".into(),
            video_api_key: "".into(),
            mode: crate::license_store::UserMode::Adult,
            mode_switched_at: Some(1718234567890),
            ..Default::default()
        };
        store.save(&f).unwrap();
        let text = std::fs::read_to_string(store.path()).unwrap();
        assert!(text.contains("\"mode\": \"adult\""), "got: {text}");
        let loaded = store.load().unwrap();
        assert_eq!(loaded.mode, crate::license_store::UserMode::Adult);
        assert_eq!(loaded.mode_switched_at, Some(1718234567890));
    }

    #[test]
    fn load_returns_none_when_file_missing() {
        let dir = tempdir().unwrap();
        let store = LicenseStore::new(dir.path());
        assert!(store.load().is_none());
    }

    #[test]
    fn load_returns_none_for_corrupt_file() {
        let dir = tempdir().unwrap();
        let store = LicenseStore::new(dir.path());
        fs::write(store.path(), b"not json").unwrap();
        assert!(store.load().is_none());
    }

    #[test]
    fn delete_removes_file() {
        let dir = tempdir().unwrap();
        let store = LicenseStore::new(dir.path());
        store
            .save(&LicenseFile {
                device_id: "x".into(),
                license_token: "y".into(),
                llm_api_key: "a".into(),
                video_api_key: "b".into(),
                ..Default::default()
            })
            .unwrap();
        assert!(store.path().exists());
        store.delete().unwrap();
        assert!(!store.path().exists());
    }
}
