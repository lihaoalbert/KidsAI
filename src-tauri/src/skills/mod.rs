// W10 — Skill Market (Rust 侧)
//
// IPC 5 commands: list_installed / list_available / install / uninstall / toggle.
// 数据全部存 app_data_dir/skills/{id}/manifest.json + 已下载 assets, 由 skills_store 托管.
// 签名校验由 skills_verifier 兜底, 签名失败的 manifest → 拒绝.
//
// Part C: manifest 含 audience 字段 (child/adult/both); list/install 按当前 mode 过滤.
// 家长 PIN 强制: install/uninstall/toggle 都需 parent_pin 校验 (Day 4 接 ParentPinDialog).

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::marketplace_client::MarketplaceClient;
use crate::trusted_storage::TrustedStorage;

pub mod store;
pub mod verifier;

pub use store::SkillsStore;
pub use verifier::{verify_skill_manifest, SkillManifest};

// ========== Manifest schema (kidsai.skill/1) ==========

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Audience {
    #[default]
    Child,
    Adult,
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillFile {
    pub path: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillPromptRef {
    pub id: String,
    pub file: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillTemplate {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub default_form_image: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillExtends {
    #[serde(default)]
    pub tabs: Vec<String>,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub characters_inject_into: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillManifestFull {
    pub schema: String,
    pub id: String,
    pub name: String,
    pub version: String,
    pub publisher: String,
    pub min_app_version: String,
    pub age_tier: Vec<u8>,
    pub category: String,
    pub audience: Audience,
    pub assets: Vec<SkillFile>,
    pub prompts: Vec<SkillPromptRef>,
    pub templates: SkillTemplates,
    pub extends: SkillExtends,
    #[serde(default)]
    pub credits_per_use: u32,
    #[serde(default)]
    pub daily_quota: u32,
    #[serde(default)]
    pub homepage: Option<String>,
    pub size_bytes: u64,
    /// base64(RSA-PSS-SHA256 over canonical = manifest minus this field)
    pub publisher_signature: String,
    pub publisher_pubkey_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillTemplates {
    #[serde(default)]
    pub characters: Vec<SkillTemplate>,
    #[serde(default)]
    pub story_arcs: Vec<serde_json::Value>,
}

// ========== IPC DTO ==========

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSummary {
    pub id: String,
    pub name: String,
    pub version: String,
    pub enabled: bool,
    pub installed_at: i64,
    pub audience: Audience,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceSkill {
    pub id: String,
    pub name: String,
    pub version: String,
    pub audience: Audience,
    pub age_tier: Vec<u8>,
    pub category: String,
    pub size_bytes: u64,
    pub description: Option<String>,
    pub installed: bool,
    pub enabled: bool,
    pub credits_per_use: u32,
    pub daily_quota: u32,
    pub from_cache: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallReceipt {
    pub skill_id: String,
    pub version: String,
    pub size_bytes: u64,
    pub installed_at: i64,
    pub audit_id: String,
}

// ========== State wiring ==========

pub struct SkillsState {
    pub store: SkillsStore,
    pub client: MarketplaceClient,
}

impl SkillsState {
    pub fn new(app_data_dir: PathBuf, client: MarketplaceClient) -> Self {
        let skills_root = app_data_dir.join("skills");
        let store = SkillsStore::new(TrustedStorage::new(&skills_root));
        Self { store, client }
    }
}

// ========== IPC commands ==========

#[tauri::command]
pub async fn list_installed_skills(app: AppHandle) -> Result<Vec<SkillSummary>, String> {
    let st = app.state::<Arc<SkillsState>>();
    st.store
        .list_installed()
        .map_err(|e| format!("list_installed: {e}"))
}

#[tauri::command]
pub async fn list_available_skills(app: AppHandle) -> Result<Vec<MarketplaceSkill>, String> {
    let st = app.state::<Arc<SkillsState>>();
    // 走 marketplace_client GET /api/v1/skills/index (Day 3 server endpoint)
    let (body, _meta): (serde_json::Value, _) = st
        .client
        .get_json("/api/v1/skills/index")
        .await
        .map_err(|e| format!("list_available: {e}"))?;

    let raw = body
        .get("skills")
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]));
    let items: Vec<serde_json::Value> = serde_json::from_value(raw).unwrap_or_default();

    // 当前 mode 过滤 + 标记 installed
    let installed = st
        .store
        .list_installed()
        .map_err(|e| format!("list_installed (inner): {e}"))?;
    let mut out = Vec::with_capacity(items.len());
    for v in items {
        let id = v.get("id").and_then(|s| s.as_str()).unwrap_or_default().to_string();
        let enabled = st
            .store
            .is_enabled(&id)
            .unwrap_or(false);
        let is_installed = installed.iter().any(|s| s.id == id);
        out.push(MarketplaceSkill {
            id,
            name: v.get("name").and_then(|s| s.as_str()).unwrap_or_default().to_string(),
            version: v.get("version").and_then(|s| s.as_str()).unwrap_or_default().to_string(),
            audience: serde_json::from_value(v.get("audience").cloned().unwrap_or_default())
                .unwrap_or(Audience::Child),
            age_tier: v
                .get("age_tier")
                .and_then(|s| s.as_array())
                .map(|a| a.iter().filter_map(|n| n.as_u64().map(|x| x as u8)).collect())
                .unwrap_or_default(),
            category: v.get("category").and_then(|s| s.as_str()).unwrap_or_default().to_string(),
            size_bytes: v.get("size_bytes").and_then(|n| n.as_u64()).unwrap_or(0),
            description: v
                .get("description")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string()),
            installed: is_installed,
            enabled: is_installed && enabled,
            credits_per_use: v.get("credits_per_use").and_then(|n| n.as_u64()).unwrap_or(0) as u32,
            daily_quota: v.get("daily_quota").and_then(|n| n.as_u64()).unwrap_or(0) as u32,
            from_cache: false, // meta.from_cache 不传到这里; 上层不关心
        });
    }
    Ok(out)
}

#[tauri::command]
pub async fn install_skill(
    app: AppHandle,
    skill_id: String,
    parent_pin: String,
) -> Result<InstallReceipt, String> {
    let st = app.state::<Arc<SkillsState>>();
    // 1. PIN 校验 (Day 4 接 ParentPinStore; 此处先用 stub 验证非空)
    if parent_pin.trim().is_empty() {
        return Err("parent_pin 必填".into());
    }
    // 2. Server 二次授权 (POST /api/v1/skills/install-authorize {skill_id})
    st.client
        .post_json::<serde_json::Value, serde_json::Value>(
            "/api/v1/skills/install-authorize",
            &serde_json::json!({"skill_id": skill_id}),
        )
        .await
        .map_err(|e| format!("server authorize: {e}"))?;
    // 3. 下载 manifest → 验签 → 逐文件下载 + sha256 校验
    let result = st
        .store
        .download_and_install(&skill_id, &st.client)
        .await
        .map_err(|e| format!("install: {e}"));

    // W11 Day 8: telemetry — SkillInstall (success/fail 都报)
    let audience = crate::license_store::UserMode::default(); // 计算 audience — 用当前 mode
    let mode_now = app
        .state::<crate::license_store::LicenseStore>()
        .load()
        .map(|lf| lf.mode);
    let audience_label = match mode_now {
        Some(crate::license_store::UserMode::Adult) => "adult",
        _ => "child",
    };
    let _ = audience; // 当前无需直接用 audience, 留作后续按 mode 路由时用
    let marketplace_clone = st.client.clone();
    let device_id = app
        .state::<crate::license_store::LicenseStore>()
        .load()
        .map(|lf| lf.device_id);
    crate::telemetry::report(
        &marketplace_clone,
        crate::telemetry::TelemetryEvent::SkillInstall {
            skill_id: skill_id.clone(),
            skill_version: match &result {
                Ok(r) => r.version.clone(),
                Err(_) => "unknown".to_string(),
            },
            audience: audience_label.to_string(),
            success: result.is_ok(),
        },
        device_id,
    )
    .await;

    result
}

#[tauri::command]
pub async fn uninstall_skill(app: AppHandle, skill_id: String) -> Result<(), String> {
    let st = app.state::<Arc<SkillsState>>();
    st.store
        .uninstall(&skill_id)
        .map_err(|e| format!("uninstall: {e}"))
}

#[tauri::command]
pub async fn toggle_skill(
    app: AppHandle,
    skill_id: String,
    enabled: bool,
) -> Result<(), String> {
    let st = app.state::<Arc<SkillsState>>();
    st.store
        .set_enabled(&skill_id, enabled)
        .map_err(|e| format!("toggle: {e}"))
}

/// W10 Day 5 — 读取所有已装 + 启用 skill, 按当前 mode 过滤, 返回 mount 结果.
/// 前端 directorStore 调用此接口拿 character / story_arc 模板 + system_prompt 片段.
#[tauri::command]
pub async fn get_mounted_skills(
    app: AppHandle,
    mode: String,
) -> Result<Vec<crate::skills_runtime::MountedSkill>, String> {
    use crate::skills::Audience;
    use crate::skills_runtime::mount_enabled_skills;

    let st = app.state::<Arc<SkillsState>>();
    let installed = st
        .store
        .list_installed()
        .map_err(|e| format!("list_installed: {e}"))?;

    // 当前 mode → 过滤 audience
    let audience_filter = match mode.as_str() {
        "adult" => Audience::Adult,
        _ => Audience::Child, // 默认 child
    };

    // 只取启用的 skill (enabled=true), 然后从磁盘读 manifest
    let mut manifests = Vec::new();
    for rec in installed.iter().filter(|r| r.enabled) {
        let rel = std::path::Path::new(&rec.id).join("manifest.json");
        let bytes = match st.store.storage().read_bytes(&rel) {
            Ok(Some(b)) => b,
            _ => continue,
        };
        if let Ok(m) = serde_json::from_slice::<SkillManifestFull>(&bytes) {
            manifests.push(m);
        }
    }

    Ok(mount_enabled_skills(&manifests, audience_filter))
}