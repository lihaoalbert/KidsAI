// W10 Day 4 — User Mode IPC (Part C)
//
// IPC 2 commands:
//   - get_user_mode()                → 当前 mode (从 LicenseFile.mode 读)
//   - set_user_mode(mode, parent_pin) → PIN 校验 + server 同步 + 写 license.json
//
// 设计:
//   - 启动时 LicenseStore.load() 已读 mode (默认值 Child, 向后兼容老 license.json)
//   - 切 mode 流程: PIN 校验 → 调 server /api/v1/me/set-mode → 写本地 license.json
//   - server 当前 stub: PIN 非空即接受 (Day 4 stub); Day 5 接 ParentPinStore 完整 argon2 校验
//   - mode 切换不重启, 不重登; 安全词 / skill 过滤由前端 store 订阅

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::license_store::{LicenseStore, UserMode};
use crate::marketplace_client::MarketplaceClient;
use crate::parent_pin::ParentPinStore;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::license_store::{LicenseFile, LicenseStore};
    use tempfile::tempdir;

    /// 验证 mode 字段正确持久化到 LicenseFile.
    /// 注: IPC 集成测试在 Tauri runtime 里跑, 这里只验证数据层.
    #[test]
    fn user_mode_serializes_as_kebab_case() {
        let json = serde_json::to_string(&UserMode::Child).unwrap();
        assert_eq!(json, "\"child\"");
        let json = serde_json::to_string(&UserMode::Adult).unwrap();
        assert_eq!(json, "\"adult\"");
    }

    #[test]
    fn user_mode_deserializes_from_kebab_case() {
        let m: UserMode = serde_json::from_str("\"adult\"").unwrap();
        assert_eq!(m, UserMode::Adult);
    }

    #[test]
    fn user_mode_default_is_child() {
        assert_eq!(UserMode::default(), UserMode::Child);
    }

    #[test]
    fn license_file_roundtrips_mode_change() {
        let dir = tempdir().unwrap();
        let store = LicenseStore::new(dir.path());
        let mut lf = LicenseFile {
            device_id: "dev-1".into(),
            license_token: "tok".into(),
            llm_api_key: "".into(),
            video_api_key: "".into(),
            mode: UserMode::Child,
            mode_switched_at: None,
            ..Default::default()
        };
        store.save(&lf).unwrap();
        // 切到 adult
        lf.mode = UserMode::Adult;
        lf.mode_switched_at = Some(1718234567890);
        store.save(&lf).unwrap();
        // 重读
        let loaded = store.load().unwrap();
        assert_eq!(loaded.mode, UserMode::Adult);
        assert_eq!(loaded.mode_switched_at, Some(1718234567890));
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetModeResponse {
    pub device_id: String,
    pub mode: UserMode,
    pub switched_at: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum UserModeError {
    #[error("parent_pin 未设置 (请先设置 PIN)")]
    PinNotSet,
    #[error("PIN 错误")]
    WrongPin,
    #[error("license.json 不存在或损坏")]
    NoLicense,
    #[error("server 同步失败: {0}")]
    Server(String),
    #[error("本地写入失败: {0}")]
    Local(String),
}

#[tauri::command]
pub async fn get_user_mode(app: AppHandle) -> Result<UserMode, String> {
    let store = app.state::<LicenseStore>().inner();
    let lf = store
        .load()
        .ok_or_else(|| "no license loaded".to_string())?;
    Ok(lf.mode)
}

#[tauri::command]
pub async fn set_user_mode(
    app: AppHandle,
    mode: UserMode,
    parent_pin: String,
) -> Result<SetModeResponse, String> {
    // 1. PIN 校验 (本地)
    let pin_store = app.state::<ParentPinStore>().inner();
    if !pin_store.is_set() {
        return Err(UserModeError::PinNotSet.to_string());
    }
    if let Err(e) = pin_store.verify(&parent_pin) {
        return Err(match e {
            crate::parent_pin::ParentPinError::WrongPin => UserModeError::WrongPin.to_string(),
            other => other.to_string(),
        });
    }

    // 2. 加载本地 license
    let store = app.state::<LicenseStore>().inner();
    let mut lf = store
        .load()
        .ok_or_else(|| UserModeError::NoLicense.to_string())?;

    // W11 Day 8: 捕获旧 mode, 供 telemetry 上报 from→to 用 (在覆盖 lf.mode 之前读).
    let from_mode = lf.mode;

    // 3. 同步 server (server 当前 stub 接受任何 PIN; 后续会再校验 device.parent_pin_hash)
    let client = app.state::<MarketplaceClient>().inner().clone();
    let resp = client
        .post_json::<serde_json::Value, serde_json::Value>(
            "/api/v1/me/set-mode",
            &serde_json::json!({
                "mode": match mode {
                    UserMode::Child => "child",
                    UserMode::Adult => "adult",
                },
                "parent_pin_proof": parent_pin,
            }),
        )
        .await
        .map_err(|e| UserModeError::Server(e.to_string()).to_string())?;

    // 4. 写本地 license.json (即使 server 失败也应尽量保留本地状态? 不, server 是权威 — 失败就 rollback)
    let now = crate::agent::now_millis_pub();
    lf.mode = mode;
    lf.mode_switched_at = Some(now);
    store.save(&lf).map_err(|e| UserModeError::Local(e).to_string())?;

    // 5. 通知 secrets_runtime 模式切换 (下次 get() 路由到对应 profile)
    let runtime = app.state::<crate::secrets_runtime::SecretsRuntime>().inner().clone();
    runtime.set_mode(mode).await;

    // 6. 通知 telemetry 当前 mode (影响后续 hash 脱敏)
    crate::telemetry::set_mode(mode);

    // 7. 上报 mode_switch 事件 (fire-and-forget, 失败只 eprintln)
    let device_id = lf.device_id.clone();
    crate::telemetry::report(
        &client,
        crate::telemetry::TelemetryEvent::ModeSwitch {
            from_mode: match from_mode {
                UserMode::Child => "child".to_string(),
                UserMode::Adult => "adult".to_string(),
            },
            to_mode: match mode {
                UserMode::Child => "child".to_string(),
                UserMode::Adult => "adult".to_string(),
            },
            success: true,
        },
        Some(device_id),
    )
    .await;

    let _ = resp; // 当前不需要解析
    Ok(SetModeResponse {
        device_id: lf.device_id,
        mode,
        switched_at: now,
    })
}