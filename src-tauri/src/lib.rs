// Tauri 2.0 主入口

pub mod agent;
pub mod anti_tamper; // W11 Day 8 — 反调试 + 内存 zeroize + 周期校验
pub mod assets_local;
pub mod character;
pub mod content;
pub mod creations;
pub mod db;
pub mod image_adapter; // W6 C1
pub mod key_pool; // Token Plan task #31: MiniMax key pool + 失败转移
pub mod levels;
pub mod license_client;
pub mod license_signer; // W10/W11 共享底座 — RSA-PSS 公钥验签
pub mod marketplace_client; // W10/W11 共享底座 — Bearer HTTPS + retry + offline cache
pub mod license_store;
pub mod model;
pub mod model_factory;
pub mod model_mock;
pub mod model_openai;
pub mod music_adapter; // W6 C3
pub mod parent_pin; // W10 Day 4 — argon2 hash, app_data_dir/parent_pin.json
pub mod projects;
pub mod prompt_builder; // W4.6 #1 — Seedance 翻译层
pub mod safety;
pub mod secrets; // W11 Day 6 — SecretCipher (HKDF + AES-GCM) + verify_and_decrypt
pub mod secrets_ipc; // W11 Day 7 — 4 IPC commands (get/check/apply/rollback)
pub mod secrets_loader; // W11 Day 7 — 启动期 bootstrap
pub mod secrets_runtime; // W11 Day 7 — SecretsRuntime 单例 + get(path) + fallback
pub mod secrets_store; // W11 Day 7 — TrustedStorage wrapper (manifest + bundle + history)
pub mod skills; // W10 — Skill Market (manifest schema + store + verifier + 5 IPC)
pub mod skills_runtime; // W10 Day 5 — Skill mount 解释器 (system_prompt + characters)
pub mod style;
pub mod telemetry; // W11 Day 8 — 按 user_mode 分桶上报
pub mod tools;
pub mod trusted_storage; // W10/W11 共享底座 — 原子写 + chmod 600
pub mod types;
pub mod user_mode; // W10 Day 4 — User Mode IPC (Part C)
pub mod video_adapter;
pub mod voice_adapter; // W6 C2

pub mod test_helpers;

// 供 integration tests 引用
pub use crate::db::Db;
pub use crate::types::LevelStatus;

pub mod crashlog;
pub mod kernel; // Agent 内核骨架 (Day 1-2) — EventBus + MemoryBus + ToolBus

use tauri::{AppHandle, Manager};

use crate::agent::{cancel_agent, run_agent, SessionRegistry};
use crate::assets_local::{download_asset, resolve_asset, AssetsLocal, TauriAssetEventSink};
use crate::character::{builtin_characters, Character, CharacterRegistry};
use crate::creations::{list_creations, save_creation};
use crate::levels::{
    completed_level_ids, get_level, list_levels, list_progress, start_level, submit_level,
    LevelStore,
};
use crate::license_client::{ActivateResponse, BalanceResponse, LicenseClient, RefreshResponse};
use crate::license_store::{LicenseFile, LicenseStore};
use crate::projects::{
    create_project, delete_project, list_projects, load_project, rename_project,
    save_project_state, Projects,
};
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
    let resp = client
        .activate(&fingerprint_hash, &nickname, age_tier)
        .await?;

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
        ..Default::default()
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
    let mut lf = store.load().ok_or_else(|| "not activated".to_string())?;
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
            let projects =
                Projects::new(&data_dir).expect("failed to initialize projects directory");
            let asset_sink = std::sync::Arc::new(TauriAssetEventSink::new(app.handle().clone()));
            let assets_local = AssetsLocal::new(&data_dir, &db_path, asset_sink)
                .expect("failed to initialize local asset downloads");
            app.manage(db);
            app.manage(projects);
            app.manage(assets_local);

            // W4.5 B2: license store + license client (server 模式按 KIDSAI_SERVER_URL env)
            let license_store = LicenseStore::new(&data_dir);
            let existing_token: Option<String> = license_store
                .load()
                .map(|lf| lf.license_token);
            let license_client = LicenseClient::from_env();
            elog!(
                "[license] mode = {} (KIDSAI_SERVER_URL={:?})",
                if license_client.is_demo() {
                    "demo"
                } else {
                    "server"
                },
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

            // W10/W11: 加载 RSA-PSS 公钥 — skills/secrets manifest 验签用.
            // 失败不阻塞启动 (demo 模式无 signing 也允许), 仅记日志.
            // Day 17 P0-1: KernelState — PetEngine / IdentityService 单例.
            // 与其他 managed service 同模式; bootstrap 失败走 Unavailable fallback.
            let kernel_state = crate::kernel::state::KernelState::bootstrap(&data_dir);
            crate::kernel::ipc::spawn_pet_mood_bridge(app.handle().clone());
            app.manage(kernel_state);

            match crate::license_signer::LicenseSigner::init_from_env() {
                Ok(()) => elog!(
                    "[signer] loaded pubkey id = {}",
                    crate::license_signer::LicenseSigner::get()
                        .map(|s| s.pubkey_id().to_string())
                        .unwrap_or_default()
                ),
                Err(e) => eprintln!("[signer] init failed (will skip signature checks): {e}"),
            }

            // W10: Skills state — TrustedStorage + MarketplaceClient + 5 IPC handlers.
            // 在 server 模式下, MarketplaceClient 启动期不带 token; LicenseStore 加载后注入.
            use std::sync::Arc;
            let marketplace_cache_dir = data_dir.join("marketplace_cache");
            let _ = std::fs::create_dir_all(&marketplace_cache_dir);
            let marketplace_client = crate::marketplace_client::MarketplaceClient::from_env(marketplace_cache_dir);
            elog!(
                "[marketplace] mode = {} (KIDSAI_SERVER_URL={:?})",
                marketplace_client.mode_label(),
                std::env::var("KIDSAI_SERVER_URL").ok()
            );
            // 注入现有 license_token (W10 Day 4 — 否则 /me/set-mode 等鉴权接口会 401)
            if let Some(token) = existing_token.clone() {
                let client_clone = marketplace_client.clone();
                // setup 是 sync 闭包, 用 tauri 的 async runtime 跑一下.
                tauri::async_runtime::spawn(async move {
                    client_clone.set_token(Some(token)).await;
                });
            }
            let skills_state = Arc::new(
                crate::skills::SkillsState::new(data_dir.clone(), marketplace_client.clone())
            );
            app.manage(skills_state);
            app.manage(marketplace_client);

            // W10 Day 4 — ParentPinStore (argon2 hash, app_data_dir/parent_pin.json)
            let parent_pin_store = crate::parent_pin::ParentPinStore::new(&data_dir);
            elog!(
                "[parent_pin] set = {}",
                parent_pin_store.is_set()
            );
            app.manage(parent_pin_store);

            // W11 Day 7 — Secrets store + runtime + bootstrap.
            // SecretsStore 始终创建 (无论 server 是否可达); runtime 默认空, 走 fallback.
            // bootstrap 失败不阻塞启动 — fallback 兜底.
            let secrets_store = crate::secrets_store::SecretsStore::new(&data_dir);
            let secrets_runtime = crate::secrets_runtime::SecretsRuntime::new();
            // 同步当前 user_mode 到 runtime (从 license.json; 老 license 默认 Child)
            {
                let ls = crate::license_store::LicenseStore::new(&data_dir);
                if let Some(lf) = ls.load() {
                    let rt = secrets_runtime.clone();
                    tauri::async_runtime::spawn(async move {
                        rt.set_mode(lf.mode).await;
                    });
                }
            }
            // 用现有 license_token 派生 KEK (无 token 时走 fallback)
            let boot_report = crate::secrets_loader::bootstrap_with_token(
                &secrets_store,
                existing_token.as_deref(),
                &secrets_runtime,
            );
            elog!(
                "[secrets] bootstrap: child={} ({}), adult={} ({}), errors={}",
                boot_report.child_loaded,
                boot_report.child_version.as_deref().unwrap_or("none"),
                boot_report.adult_loaded,
                boot_report.adult_version.as_deref().unwrap_or("none"),
                boot_report.errors.len()
            );
            for e in &boot_report.errors {
                eprintln!("[secrets_loader] {e}");
            }
            app.manage(secrets_store);
            app.manage(secrets_runtime);

            // W11 Day 8: Telemetry — init mode from license, 然后准备上报器.
            {
                let ls = crate::license_store::LicenseStore::new(&data_dir);
                if let Some(lf) = ls.load() {
                    crate::telemetry::set_mode(lf.mode);
                }
            }

            // W11 Day 8: 反调试 — 启动期 check + 后台 30 min 周期校验.
            crate::anti_tamper::startup_check();
            let shutdown = std::sync::Arc::new(crate::anti_tamper::ArcShutdown::new());
            crate::anti_tamper::spawn_periodic_check(shutdown.clone());

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
            // 项目 + 本地资产
            list_projects,
            load_project,
            create_project,
            rename_project,
            delete_project,
            save_project_state,
            download_asset,
            resolve_asset,
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
            // Skills（W10 Day 3）
            crate::skills::list_installed_skills,
            crate::skills::list_available_skills,
            crate::skills::install_skill,
            crate::skills::uninstall_skill,
            crate::skills::toggle_skill,
            crate::skills::get_mounted_skills,
            // Parent PIN + User Mode (W10 Day 4 — Part C)
            crate::parent_pin::set_parent_pin,
            crate::parent_pin::verify_parent_pin,
            crate::parent_pin::is_parent_pin_set,
            crate::parent_pin::reset_parent_pin,
            crate::user_mode::get_user_mode,
            crate::user_mode::set_user_mode,
            // Secrets IPC (W11 Day 7)
            crate::secrets_ipc::get_current_secret_version,
            crate::secrets_ipc::check_secrets_update,
            crate::secrets_ipc::apply_secrets_update,
            crate::secrets_ipc::rollback_secrets,
            // P1-5: 多版本回滚 UI 配套 — 列出 history
            crate::secrets_ipc::list_secret_versions,
            // Kernel IPC (Day 17 P0-1: PetEngine 接通 + Identity)
            crate::kernel::ipc::pet_tick,
            crate::kernel::ipc::save_identity,
            crate::kernel::ipc::load_identity,
            crate::kernel::ipc::bump_last_seen,
        ])
        .run(tauri::generate_context!())
        .expect("启动 KidsAI Studio 时出错");
}
