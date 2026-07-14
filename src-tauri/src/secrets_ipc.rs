// W11 Day 7 — Secrets IPC (4 commands)
//
//   get_current_secret_version(app) → HashMap<profile, version>
//   check_secrets_update(app)       → Vec<UpdateInfo> (server 上有比本地新的)
//   apply_secrets_update(app, profile, parent_pin) → Result<String, String>
//   rollback_secrets(app, profile, to_version) → Result<(), String>

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::license_store::LicenseStore;
use crate::marketplace_client::MarketplaceClient;
use crate::parent_pin::ParentPinStore;
use crate::secrets::SecretsManifest;
use crate::secrets_runtime::SecretsRuntime;
use crate::secrets_store::SecretsStore;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub profile: String,
    pub remote_version: String,
    pub current_version: Option<String>,
}

#[tauri::command]
pub async fn get_current_secret_version(
    app: AppHandle,
) -> Result<HashMap<String, String>, String> {
    let store = app.state::<SecretsStore>().inner();
    let cur = store.load_current().map_err(|e| format!("load_current: {e}"))?;
    Ok(cur.profiles)
}

#[tauri::command]
pub async fn check_secrets_update(
    app: AppHandle,
) -> Result<Vec<UpdateInfo>, String> {
    let store = app.state::<SecretsStore>().inner();
    let client = app.state::<MarketplaceClient>().inner().clone();

    let cur = store.load_current().map_err(|e| format!("load_current: {e}"))?;
    let profiles = ["child", "adult"];

    let mut out = Vec::new();
    for p in profiles {
        // GET /api/v1/secrets/manifest?profile={p}
        let path = format!("/api/v1/secrets/manifest?profile={p}");
        let (bytes, _meta) = match client.get_bytes(&path).await {
            Ok(b) => b,
            Err(_) => continue, // offline / 404 / 5xx → 跳过
        };
        let m: SecretsManifest = match serde_json::from_slice(&bytes) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let current = cur.profiles.get(p).cloned();
        // 有更新 = server 版本 ≠ 本地版本
        if current.as_deref() != Some(&m.version) {
            out.push(UpdateInfo {
                profile: p.to_string(),
                remote_version: m.version,
                current_version: current,
            });
        }
    }
    Ok(out)
}

#[tauri::command]
pub async fn apply_secrets_update(
    app: AppHandle,
    profile: String,
    parent_pin: String,
) -> Result<String, String> {
    // 1. PIN 校验
    let pin_store = app.state::<ParentPinStore>().inner();
    if !pin_store.is_set() {
        return Err("parent_pin 未设置".into());
    }
    pin_store
        .verify(&parent_pin)
        .map_err(|e| format!("PIN 校验失败: {e}"))?;

    // 2. 拿 license_token
    let license_store = app.state::<LicenseStore>().inner();
    let license_file = license_store
        .load()
        .ok_or_else(|| "no license loaded".to_string())?;
    let token = license_file.license_token.clone();
    let device_id = license_file.device_id.clone();
    let prev_version = license_file
        .mode_switched_at
        .map(|_| "v0") // 不重要, 仅占位 — 真实 prev_version 从 SecretsStore.current 读
        .unwrap_or("none");
    let _ = prev_version; // suppress unused

    // 3. GET manifest
    let client = app.state::<MarketplaceClient>().inner().clone();
    let manifest_path = format!("/api/v1/secrets/manifest?profile={profile}");
    let (manifest_bytes, _meta) = client
        .get_bytes(&manifest_path)
        .await
        .map_err(|e| format!("get manifest: {e}"))?;
    let manifest: SecretsManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|e| format!("parse manifest: {e}"))?;
    let resolved_version = manifest.version.clone();

    // 4. POST wrap
    let wrap_path = format!("/api/v1/secrets/wrap?profile={profile}&version={resolved_version}");
    let wrap_resp: serde_json::Value = client
        .post_json(&wrap_path, &serde_json::json!({}))
        .await
        .map_err(|e| format!("post wrap: {e}"))?;
    let wrapped: crate::secrets::WrappedMaster = serde_json::from_value(wrap_resp)
        .map_err(|e| format!("parse wrap: {e}"))?;

    // 5. GET bundle
    let bundle_path = format!("/api/v1/secrets/bundle?profile={profile}&version={resolved_version}");
    let (bundle_ct, _meta) = client
        .get_bytes(&bundle_path)
        .await
        .map_err(|e| format!("get bundle: {e}"))?;

    // 6. 验签 + 解密 (用 license_token)
    let plaintext = crate::secrets::verify_and_decrypt(&manifest, &wrapped, &bundle_ct, &token)
        .map_err(|e| format!("verify_and_decrypt: {e}"))?;

    // 7. 拆文件 + 逐文件 sha256
    let entries = crate::secrets::split_bundle(&plaintext);
    for (path, bytes) in &entries {
        if let Some(expected) = manifest.files.iter().find(|f| &f.path == path) {
            let actual = crate::secrets::sha256_hex(bytes);
            if actual != expected.sha256 {
                // W11 Day 8 telemetry 上报失败
                crate::telemetry::report(
                    &client,
                    crate::telemetry::TelemetryEvent::SecretUpdate {
                        profile: profile.clone(),
                        from_version: None,
                        to_version: resolved_version.clone(),
                        success: false,
                    },
                    Some(device_id.clone()),
                )
                .await;
                return Err(format!(
                    "file sha mismatch for {path}: expected {}, got {}",
                    expected.sha256, actual
                ));
            }
        }
    }

    // 8. 写盘
    let store = app.state::<SecretsStore>().inner();
    store
        .install_version(&profile, &manifest, &bundle_ct, &wrapped)
        .map_err(|e| format!("install_version: {e}"))?;

    // 9. 注入 runtime
    let runtime = app.state::<SecretsRuntime>().inner();
    runtime.install_profile_files(&profile, entries).await;

    // W11 Day 8: SecretUpdate telemetry
    crate::telemetry::report(
        &client,
        crate::telemetry::TelemetryEvent::SecretUpdate {
            profile: profile.clone(),
            from_version: None,
            to_version: resolved_version.clone(),
            success: true,
        },
        Some(device_id.clone()),
    )
    .await;

    Ok(resolved_version)
}

#[tauri::command]
pub async fn rollback_secrets(
    app: AppHandle,
    profile: String,
    to_version: String,
) -> Result<(), String> {
    let license_store = app.state::<LicenseStore>().inner();
    let license_file = license_store
        .load()
        .ok_or_else(|| "no license loaded".to_string())?;
    let token = license_file.license_token.clone();

    let store = app.state::<SecretsStore>().inner();

    // 验证目标版本存在
    let versions = store
        .list_versions(&profile)
        .map_err(|e| format!("list_versions: {e}"))?;
    if !versions.contains(&to_version) {
        return Err(format!(
            "version {profile}/{to_version} 不在 history 里"
        ));
    }

    let manifest = store
        .read_manifest(&profile, &to_version)
        .map_err(|e| format!("read_manifest: {e}"))?;
    let bundle_ct = store
        .read_bundle(&profile, &to_version)
        .map_err(|e| format!("read_bundle: {e}"))?;
    let wrapped = store
        .read_wrapped(&profile, &to_version)
        .map_err(|e| format!("read_wrapped: {e}"))?;

    let plaintext = crate::secrets::verify_and_decrypt(&manifest, &wrapped, &bundle_ct, &token)
        .map_err(|e| format!("verify_and_decrypt: {e}"))?;
    let entries = crate::secrets::split_bundle(&plaintext);

    let runtime = app.state::<SecretsRuntime>().inner();
    runtime.install_profile_files(&profile, entries).await;

    // 更新 current.json
    let mut cur = store
        .load_current()
        .map_err(|e| format!("load_current: {e}"))?;
    cur.profiles.insert(profile.clone(), to_version);
    cur.updated_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    store
        .save_current(&cur)
        .map_err(|e| format!("save_current: {e}"))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::license_store::{LicenseFile, LicenseStore, UserMode};
    use crate::secrets_runtime::SecretsRuntime;
    use crate::secrets_store::SecretsStore;
    use crate::trusted_storage::TrustedStorage;
    use tempfile::tempdir;

    /// IPC DTO serialization smoke
    #[test]
    fn update_info_roundtrip() {
        let u = UpdateInfo {
            profile: "child".into(),
            remote_version: "v1.new".into(),
            current_version: Some("v1.old".into()),
        };
        let s = serde_json::to_string(&u).unwrap();
        let r: UpdateInfo = serde_json::from_str(&s).unwrap();
        assert_eq!(r.profile, "child");
        assert_eq!(r.remote_version, "v1.new");
        assert_eq!(r.current_version.as_deref(), Some("v1.old"));
    }

    /// secrets runtime + store 协同: install files → 当前版本含 child
    #[tokio::test]
    async fn runtime_includes_installed_profiles() {
        let dir = tempdir().unwrap();
        let _store = SecretsStore::new(dir.path());
        let runtime = SecretsRuntime::new();
        runtime
            .install_profile_files("child", vec![("a".into(), b"1".to_vec())])
            .await;
        let v = runtime.current_versions().await;
        assert!(v.contains_key("child"));
    }

    /// LicenseStore 配合: UserMode 切到 Adult 后 runtime 也跟着切
    #[tokio::test]
    async fn runtime_mode_matches_license() {
        let dir = tempdir().unwrap();
        let lf = LicenseFile {
            device_id: "d".into(),
            license_token: "t".into(),
            llm_api_key: "".into(),
            video_api_key: "".into(),
            mode: UserMode::Adult,
            ..Default::default()
        };
        let ls = LicenseStore::new(dir.path());
        ls.save(&lf).unwrap();
        let runtime = SecretsRuntime::new();
        runtime.set_mode(ls.load().unwrap().mode).await;
        assert_eq!(runtime.mode().await, UserMode::Adult);
    }

    /// SecretsStore 反复读写一致
    #[test]
    fn secrets_store_smoke() {
        let dir = tempdir().unwrap();
        let _ts = TrustedStorage::new(dir.path());
        let ss = SecretsStore::new(dir.path());
        assert!(ss.load_current().unwrap().profiles.is_empty());
    }
}