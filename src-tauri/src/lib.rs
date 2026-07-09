// Tauri 2.0 主入口

mod agent;
mod content;
mod creations;
mod db;
mod levels;
mod types;

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

#[tauri::command]
fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("你好，{}！欢迎来到 KidsAI Studio 🦉", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            // 打开 SQLite 数据库（W2.3）
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app data dir");
            let db_path = data_dir.join("kidsai.db");
            eprintln!("[db] opening at {:?}", db_path);
            let db = Db::open(&db_path).expect("failed to open SQLite database");
            app.manage(db);

            // UI 标题
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
            // 作品（W2.3）
            save_creation,
            list_creations,
        ])
        .run(tauri::generate_context!())
        .expect("启动 KidsAI Studio 时出错");
}
