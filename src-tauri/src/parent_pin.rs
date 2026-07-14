// W10 Day 4 — Parent PIN 存储 + 校验
//
// 流程:
//   1. 家长首次启用 ModeSwitchDialog / Marketplace 时, 弹 ParentPinSetup 输入 4 位 PIN
//   2. argon2id hash + salt 存 app_data_dir/parent_pin.json (chmod 600, 单 device 一份)
//   3. 后续 ModeSwitch / Install / Uninstall 都要求输入 PIN → verify
//
// 安全模型:
//   - PIN 仅本地校验 (不进 server); server 端在 install-authorize 时再做 device.parent_authorized 二次确认 (Day 4 stub 接 device.parent_pin_hash 列)
//   - argon2id 默认参数: m=19456, t=2, p=1 (OWASP 推荐, 单核 100ms 量级)
//   - 防暴力破解: 5 次错误 → 锁 60 秒 (前端状态, 不在 store 里)
//
// 与 license_token 的关系:
//   - parent_pin 是 device-local 凭证, 不与 license_token 绑定
//   - 设备迁移: 重装 KidsAI 时 PIN 丢失, 需重设 (设计取舍: 不上传 PIN 到 server)

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use serde::{Deserialize, Serialize};
use tauri::Manager;

const PIN_FILENAME: &str = "parent_pin.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParentPinFile {
    /// argon2id PHC string (含 algorithm/params/salt/hash)
    pub pin_hash: String,
    /// 设置时间 (ms)
    pub created_at: i64,
    /// 上次成功验证时间 (ms)
    pub last_verified_at: Option<i64>,
}

#[derive(Debug, thiserror::Error)]
pub enum ParentPinError {
    #[error("PIN 文件读写失败: {0}")]
    Io(#[from] std::io::Error),
    #[error("PIN 序列化失败: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("argon2: {0}")]
    Argon2(String),
    #[error("PIN 错误")]
    WrongPin,
    #[error("PIN 格式无效 (需 4-8 位数字)")]
    InvalidFormat,
    #[error("PIN 未设置 (请先设置)")]
    NotSet,
}

impl From<argon2::password_hash::Error> for ParentPinError {
    fn from(e: argon2::password_hash::Error) -> Self {
        ParentPinError::Argon2(format!("{e}"))
    }
}

pub struct ParentPinStore {
    path: PathBuf,
}

impl ParentPinStore {
    pub fn new(app_data_dir: &Path) -> Self {
        Self {
            path: app_data_dir.join(PIN_FILENAME),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn is_set(&self) -> bool {
        self.path.exists()
    }

    pub fn load(&self) -> Result<Option<ParentPinFile>, ParentPinError> {
        if !self.path.exists() {
            return Ok(None);
        }
        let text = fs::read_to_string(&self.path)?;
        let f: ParentPinFile = serde_json::from_str(&text)?;
        Ok(Some(f))
    }

    /// 设置 / 覆盖 PIN. 强制 4-8 位数字.
    pub fn set(&self, pin: &str) -> Result<(), ParentPinError> {
        validate_pin(pin)?;
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let phc = argon2
            .hash_password(pin.as_bytes(), &salt)
            .map_err(|e| ParentPinError::Argon2(format!("hash: {e}")))?
            .to_string();

        let file = ParentPinFile {
            pin_hash: phc,
            created_at: now_millis(),
            last_verified_at: None,
        };
        self.write(&file)
    }

    /// 验证 PIN. 成功 → 更新 last_verified_at, 失败 → WrongPin.
    pub fn verify(&self, pin: &str) -> Result<(), ParentPinError> {
        let mut file = self
            .load()?
            .ok_or(ParentPinError::NotSet)?;
        let parsed = PasswordHash::new(&file.pin_hash)
            .map_err(|e| ParentPinError::Argon2(format!("parse phc: {e}")))?;
        Argon2::default()
            .verify_password(pin.as_bytes(), &parsed)
            .map_err(|_| ParentPinError::WrongPin)?;
        file.last_verified_at = Some(now_millis());
        self.write(&file)?;
        Ok(())
    }

    /// 重置 (家长忘记 PIN 时: 先 verify 当前 PIN 才能 reset, 或者 admin 端单独命令).
    /// 当前简化: reset 直接清空文件, 下次操作时再 set.
    pub fn reset(&self) -> Result<(), ParentPinError> {
        if self.path.exists() {
            fs::remove_file(&self.path)?;
        }
        Ok(())
    }

    fn write(&self, file: &ParentPinFile) -> Result<(), ParentPinError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(file)?;
        let tmp = self.path.with_extension("json.tmp");
        {
            let mut f = fs::File::create(&tmp)?;
            f.write_all(json.as_bytes())?;
            f.sync_all()?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
        }
        fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

fn validate_pin(pin: &str) -> Result<(), ParentPinError> {
    if pin.len() < 4 || pin.len() > 8 {
        return Err(ParentPinError::InvalidFormat);
    }
    if !pin.chars().all(|c| c.is_ascii_digit()) {
        return Err(ParentPinError::InvalidFormat);
    }
    Ok(())
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
    use tempfile::tempdir;

    #[test]
    fn new_store_not_set() {
        let dir = tempdir().unwrap();
        let store = ParentPinStore::new(dir.path());
        assert!(!store.is_set());
        assert!(matches!(store.load().unwrap(), None));
    }

    #[test]
    fn set_then_verify_succeeds() {
        let dir = tempdir().unwrap();
        let store = ParentPinStore::new(dir.path());
        store.set("1234").unwrap();
        assert!(store.is_set());
        store.verify("1234").unwrap();
    }

    #[test]
    fn verify_wrong_pin_errors() {
        let dir = tempdir().unwrap();
        let store = ParentPinStore::new(dir.path());
        store.set("1234").unwrap();
        assert!(matches!(store.verify("5678"), Err(ParentPinError::WrongPin)));
    }

    #[test]
    fn verify_unset_errors() {
        let dir = tempdir().unwrap();
        let store = ParentPinStore::new(dir.path());
        assert!(matches!(
            store.verify("1234"),
            Err(ParentPinError::NotSet)
        ));
    }

    #[test]
    fn set_rejects_non_digit() {
        let dir = tempdir().unwrap();
        let store = ParentPinStore::new(dir.path());
        assert!(matches!(
            store.set("abcd"),
            Err(ParentPinError::InvalidFormat)
        ));
        assert!(matches!(
            store.set("12a4"),
            Err(ParentPinError::InvalidFormat)
        ));
    }

    #[test]
    fn set_rejects_too_short() {
        let dir = tempdir().unwrap();
        let store = ParentPinStore::new(dir.path());
        assert!(matches!(
            store.set("123"),
            Err(ParentPinError::InvalidFormat)
        ));
    }

    #[test]
    fn set_rejects_too_long() {
        let dir = tempdir().unwrap();
        let store = ParentPinStore::new(dir.path());
        assert!(matches!(
            store.set("123456789"),
            Err(ParentPinError::InvalidFormat)
        ));
    }

    #[test]
    fn set_overwrites_previous() {
        let dir = tempdir().unwrap();
        let store = ParentPinStore::new(dir.path());
        store.set("1234").unwrap();
        store.set("5678").unwrap();
        store.verify("5678").unwrap();
        assert!(matches!(store.verify("1234"), Err(ParentPinError::WrongPin)));
    }

    #[test]
    fn reset_clears_pin() {
        let dir = tempdir().unwrap();
        let store = ParentPinStore::new(dir.path());
        store.set("1234").unwrap();
        store.reset().unwrap();
        assert!(!store.is_set());
        assert!(matches!(
            store.verify("1234"),
            Err(ParentPinError::NotSet)
        ));
    }

    #[test]
    fn set_then_load_returns_file() {
        let dir = tempdir().unwrap();
        let store = ParentPinStore::new(dir.path());
        store.set("12345678").unwrap();
        let f = store.load().unwrap().unwrap();
        assert!(f.pin_hash.starts_with("$argon2id$"));
        assert!(f.created_at > 0);
    }
}

// ============ IPC ============

#[tauri::command]
pub fn is_parent_pin_set(app: tauri::AppHandle) -> bool {
    let store = app.state::<ParentPinStore>().inner();
    store.is_set()
}

#[tauri::command]
pub fn set_parent_pin(app: tauri::AppHandle, pin: String) -> Result<(), String> {
    let store = app.state::<ParentPinStore>().inner();
    store.set(&pin).map_err(|e| format!("{e}"))
}

#[tauri::command]
pub fn verify_parent_pin(app: tauri::AppHandle, pin: String) -> Result<bool, String> {
    let store = app.state::<ParentPinStore>().inner();
    match store.verify(&pin) {
        Ok(()) => Ok(true),
        Err(crate::parent_pin::ParentPinError::WrongPin) => Ok(false),
        Err(e) => Err(format!("{e}")),
    }
}

#[tauri::command]
pub fn reset_parent_pin(app: tauri::AppHandle) -> Result<(), String> {
    let store = app.state::<ParentPinStore>().inner();
    store.reset().map_err(|e| format!("{e}"))
}