// Tauri 2.0 主入口

pub mod agent;
pub mod character;
pub mod content;
pub mod creations;
pub mod db;
pub mod image_adapter;   // W6 C1
pub mod levels;
pub mod license_client;
pub mod license_store;
pub mod model;
pub mod model_factory;
pub mod model_mock;
pub mod model_openai;
pub mod music_adapter;   // W6 C3
pub mod safety;
pub mod style;
pub mod tools;
pub mod types;
pub mod video_adapter;
pub mod voice_adapter;   // W6 C2

pub mod test_helpers;

// 供 integration tests 引用
pub use crate::db::Db;
pub use crate::types::LevelStatus;

pub mod crashlog;

use tauri::{AppHandle, Manager};

use crate::agent::{cancel_agent, run_agent, SessionRegistry};
use crate::character::{builtin_characters, Character, CharacterRegistry};
use crate::creations::{list_creations, save_creation};
use crate::levels::{
    completed_level_ids, get_level, list_levels, list_progress, start_level, submit_level,
    LevelStore,
};
use crate::license_client::{ActivateResponse, BalanceResponse, LicenseClient, RefreshResponse};
use crate::license_store::{LicenseFile, LicenseStore};
use crate::safety::{KeywordFilter, SafetyVerdict};
use crate::style::{builtin_styles, StylePreset, StyleRegistry};

#[tauri::command]
fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("你好，{}！欢迎来到 KidsAI Studio 🦉", name)
}

/// 内容审核命令（W2.7）
/// 前端在收到用户输入后、提交给 Agent 前先调一次
#[tauri::command]
fn check_safety(text: String) -> SafetyVerdict {
    KeywordFilter::new().check(&text)
}

/// 当前模型来源（W3.1）
/// 前端可以展示给用户："当前由 deepseek 提供"
#[tauri::command]
fn current_model_source() -> String {
    let _ = dotenvy::dotenv();
    crate::model_factory::select_model().source
}

/// W4.5 B2: 设备激活 — 调 server, 写 license.json, 返回 license 给前端存 store
#[tauri::command]
async fn activate_device(
    app: AppHandle,
    fingerprint_hash: String,
    nickname: String,
    age_tier: u8,
) -> Result<ActivateResponse, String> {
    let client = app.state::<LicenseClient>().inner().clone();
    let resp = client.activate(&fingerprint_hash, &nickname, age_tier).await?;

    let store = app.state::<LicenseStore>().inner();
    let license_file = LicenseFile {
        device_id: resp.device_id.clone(),
        license_token: resp.license_token.clone(),
        llm_api_key: resp.api_keys.llm.clone(),
        video_api_key: resp.api_keys.video.clone(),
        last_balance: Some(resp.balance),
        nickname: Some(nickname),
        age_tier: Some(age_tier),
        activated_at: Some(crate::agent::now_millis_pub()),
    };
    store.save(&license_file)?;
    Ok(resp)
}

/// W4.5 B2: 查余额 (前端 HomePage 学币栏)
#[tauri::command]
async fn get_balance(app: AppHandle) -> Result<BalanceResponse, String> {
    let client = app.state::<LicenseClient>().inner().clone();
    let store = app.state::<LicenseStore>().inner();
    let lf = store
        .load()
        .ok_or_else(|| "not activated, run activate_device first".to_string())?;
    client.get_balance(&lf.license_token).await
}

/// W4.5 B2: 续签 license + 轮换 api_keys
#[tauri::command]
async fn refresh_license(app: AppHandle) -> Result<RefreshResponse, String> {
    let client = app.state::<LicenseClient>().inner().clone();
    let store = app.state::<LicenseStore>().inner();
    let mut lf = store
        .load()
        .ok_or_else(|| "not activated".to_string())?;
    let r = client.refresh_license(&lf.license_token).await?;
    lf.license_token = r.license_token.clone();
    lf.llm_api_key = r.api_keys.llm.clone();
    lf.video_api_key = r.api_keys.video.clone();
    store.save(&lf)?;
    Ok(r)
}

/// W4.5 B2: 返回当前 license 摘要 (用于前端首屏判断是否要进 Onboarding)
#[tauri::command]
fn get_license_info(app: AppHandle) -> Option<LicenseInfo> {
    let store = app.state::<LicenseStore>().inner();
    store.load().map(|lf| LicenseInfo {
        device_id: lf.device_id,
        nickname: lf.nickname.unwrap_or_default(),
        age_tier: lf.age_tier.unwrap_or(0),
        last_balance: lf.last_balance.unwrap_or(0),
        is_demo: app.state::<LicenseClient>().inner().is_demo(),
        activated_at: lf.activated_at.unwrap_or(0),
    })
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LicenseInfo {
    pub device_id: String,
    pub nickname: String,
    pub age_tier: u8,
    pub last_balance: i64,
    pub is_demo: bool,
    pub activated_at: i64,
}

/// W4.5 B2: 清空 license (退出账号 / 重置)
#[tauri::command]
fn reset_license(app: AppHandle) -> Result<(), String> {
    let store = app.state::<LicenseStore>().inner();
    store.delete()
}

/// 列出内置角色（W3.4）
#[tauri::command]
fn list_characters() -> Vec<Character> {
    builtin_characters()
}

/// 列出内置风格模板（W3.6）
#[tauri::command]
fn list_styles() -> Vec<StylePreset> {
    builtin_styles()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app data dir");

            // W4.5 D1: 崩溃日志 (写在最前, 这样后续 init 出错也能留下痕迹)
            crashlog::init(&data_dir);
            crashlog::event("setup", &format!("app_data_dir = {:?}", data_dir));

            let db_path = data_dir.join("kidsai.db");
            elog!("[db] opening at {:?}", db_path);
            let db = Db::open(&db_path).expect("failed to open SQLite database");
            app.manage(db);

            // W4.5 B2: license store + license client (server 模式按 KIDSAI_SERVER_URL env)
            let license_store = LicenseStore::new(&data_dir);
            let license_client = LicenseClient::from_env();
            elog!(
                "[license] mode = {} (KIDSAI_SERVER_URL={:?})",
                if license_client.is_demo() { "demo" } else { "server" },
                std::env::var("KIDSAI_SERVER_URL").ok()
            );
            app.manage(license_store);
            app.manage(license_client);

            // W3.4: 预填内置角色到注册表（未来允许用户上传新角色）
            let char_reg = CharacterRegistry::new();
            for c in builtin_characters() {
                char_reg.register(c);
            }
            app.manage(char_reg);

            // W3.6: 预填内置风格到注册表
            let style_reg = StyleRegistry::new();
            for s in builtin_styles() {
                style_reg.register(s);
            }
            app.manage(style_reg);

            let window = app.get_webview_window("main").unwrap();
            window.set_title("KidsAI Studio").ok();
            Ok(())
        })
        .manage(LevelStore::default())
        .manage(SessionRegistry::default())
        .invoke_handler(tauri::generate_handler![
            get_app_version,
            greet,
            // 关卡
            list_levels,
            get_level,
            list_progress,
            start_level,
            submit_level,
            completed_level_ids,
            // Agent
            run_agent,
            cancel_agent,
            // 角色（W3.4）
            list_characters,
            // 风格（W3.6）
            list_styles,
            // 作品
            save_creation,
            list_creations,
            // 安全
            check_safety,
            // 模型
            current_model_source,
            // License（W4.5 B2）
            activate_device,
            get_balance,
            refresh_license,
            get_license_info,
            reset_license,
        ])
        .run(tauri::generate_context!())
        .expect("启动 KidsAI Studio 时出错");
}
