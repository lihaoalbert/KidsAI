// Tauri 2.0 主入口

use tauri::Manager;

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
            // 启动时打印一条日志
            let window = app.get_webview_window("main").unwrap();
            window.set_title("KidsAI Studio").ok();
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_app_version, greet])
        .run(tauri::generate_context!())
        .expect("启动 KidsAI Studio 时出错");
}
