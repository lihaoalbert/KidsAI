// Tauri 2.0 主入口
// W2.2: 注册关卡 / Agent 命令骨架；W2.3 接 SQLite；W2.4 接入真实 Agent Loop

mod agent;
mod content;
mod levels;
mod types;

use tauri::Manager;

use crate::agent::run_agent;
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
        .manage(LevelStore::default())
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();
            window.set_title("KidsAI Studio").ok();
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_app_version,
            greet,
            // 关卡（W2.2）
            list_levels,
            get_level,
            list_progress,
            start_level,
            submit_level,
            completed_level_ids,
            // Agent（W2.2 占位 / W2.4 实现）
            run_agent,
        ])
        .run(tauri::generate_context!())
        .expect("启动 KidsAI Studio 时出错");
}
