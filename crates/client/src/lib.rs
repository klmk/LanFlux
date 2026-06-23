//! net-tool 客户端 / 实施端库
//!
//! 本 crate 同时提供库（供桌面端 Tauri 应用集成）和二进制（CLI）两种形式。
//!
//! ## 核心模块
//!
//! - [`connection`]：服务端连接管理（注册、心跳、状态查询）
//! - [`tunnel_client`]：隧道客户端（认证、TUN 创建、数据包转发）
//! - [`scanner`]：本机网卡扫描与网段推测
//! - [`config`]：配置文件加载与保存
//! - [`diagnostic`]：连通性诊断
//! - [`display`]：命令行表格与着色输出
//! - [`cli`]：CLI 参数定义与公共辅助函数

pub mod cli;
pub mod client_mode;
pub mod config;
pub mod connection;
pub mod diagnostic;
pub mod display;
pub mod operator_mode;
pub mod scanner;
pub mod server_mode;
pub mod tunnel_client;
