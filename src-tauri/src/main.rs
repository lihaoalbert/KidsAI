// 防止 Windows 下额外的 console 窗口
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    kidsai_studio_lib::run()
}
