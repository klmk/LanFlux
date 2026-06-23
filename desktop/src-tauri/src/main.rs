//! NetTool 桌面客户端 Tauri 入口
//!
//! 注册 Tauri 命令并启动应用窗口。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod client_manager;
mod commands;

use client_manager::ClientManager;
use commands::{
    connect_client, connect_operator, convert_ip, disconnect_all, get_accessible_segments,
    get_app_info, get_reported_segments, get_status, get_tunnel_routes, load_config, ping_test,
    refresh_accessible_segments, refresh_reported_segments, report_segment, save_config,
    scan_interfaces, start_tunnel, stop_tunnel, tcp_test, test_connectivity,
};

fn main() {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .try_init()
        .ok();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(ClientManager::new())
        .invoke_handler(tauri::generate_handler![
            // 基础命令
            scan_interfaces,
            get_app_info,
            test_connectivity,
            save_config,
            load_config,
            // 网络连接命令
            connect_client,
            connect_operator,
            start_tunnel,
            stop_tunnel,
            disconnect_all,
            get_status,
            get_tunnel_routes,
            get_accessible_segments,
            refresh_accessible_segments,
            get_reported_segments,
            refresh_reported_segments,
            report_segment,
            ping_test,
            tcp_test,
            convert_ip,
        ])
        .run(tauri::generate_context!())
        .expect("运行 Tauri 应用时出错");
}
