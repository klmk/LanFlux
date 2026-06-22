//! NetTool 桌面客户端 Tauri 入口
//!
//! 注册 Tauri 命令并启动应用窗口。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

use commands::{get_app_info, load_config, save_config, scan_interfaces, test_connectivity};

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            scan_interfaces,
            get_app_info,
            test_connectivity,
            save_config,
            load_config,
        ])
        .run(tauri::generate_context!())
        .expect("运行 Tauri 应用时出错");
}
