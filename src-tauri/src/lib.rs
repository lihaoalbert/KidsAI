// Tauri 2.0 主入口

pub mod agent;
pub mod character;
pub mod content;
pub mod creations;
pub mod db;
pub mod levels;
pub mod model;
pub mod model_factory;
pub mod model_mock;
pub mod model_openai;
pub mod safety;
pub mod style;
pub mod tools;
pub mod types;
pub mod video_adapter;

pub mod test_helpers;

// 供 integration tests 引用
pub use crate::db::Db;
pub use crate::types::LevelStatus;

use tauri::Manager;

use crate::agent::{cancel_agent, run_agent, SessionRegistry};
use crate::character::{builtin_characters, Character, CharacterRegistry};
use crate::creations::{list_creations, save_creation};
use crate::levels::{
    completed_level_ids, get_level, list_levels, list_progress, start_level, submit_level,
    LevelStore,
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
            let db_path = data_dir.join("kidsai.db");
            eprintln!("[db] opening at {:?}", db_path);
            let db = Db::open(&db_path).expect("failed to open SQLite database");
            app.manage(db);

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
        ])
        .run(tauri::generate_context!())
        .expect("启动 KidsAI Studio 时出错");
}
