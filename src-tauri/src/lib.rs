// Tauri 2.0 主入口

pub mod agent;
pub mod content;
pub mod creations;
pub mod db;
pub mod levels;
pub mod model;
pub mod model_mock;
pub mod safety;
pub mod tools;
pub mod types;

pub mod test_helpers;

// 供 integration tests 引用
pub use crate::db::Db;
pub use crate::types::LevelStatus;

use tauri::Manager;

use crate::agent::run_agent;
use crate::creations::{list_creations, save_creation};
use crate::levels::{
    completed_level_ids, get_level, list_levels, list_progress, start_level, submit_level,
    LevelStore,
};
use crate::safety::{KeywordFilter, SafetyVerdict};

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

            let window = app.get_webview_window("main").unwrap();
            window.set_title("KidsAI Studio").ok();
            Ok(())
        })
        .manage(LevelStore::default())
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
            // 作品
            save_creation,
            list_creations,
            // 安全
            check_safety,
        ])
        .run(tauri::generate_context!())
        .expect("启动 KidsAI Studio 时出错");
}
