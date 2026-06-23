//! net-tool 客户端 / 实施端 CLI 入口
//!
//! 同一个二进制支持三种运行模式：
//! - `server`：以服务端模式运行（轻量内嵌，生产环境建议使用独立服务端）
//! - `client`：以普通客户端模式运行，上报真实网段
//! - `operator`：以实施端模式运行，访问客户端网段
//!
//! 使用 [`clap`] 解析命令行参数，使用 [`tokio`] 作为异步运行时。

use anyhow::Result;
use clap::Parser;

use net_tool_client::cli::{Cli, Commands};
use net_tool_client::{client_mode, operator_mode, server_mode};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // 先用一个默认级别初始化日志，各模式内部会按配置覆盖。
    let default_level = cli
        .common_log_level()
        .unwrap_or_else(|| "info".to_string());
    net_tool_client::cli::init_tracing(&default_level);

    match cli.command {
        Commands::Server { bind, open, common } => {
            server_mode::run(bind, open, common).await
        }
        Commands::Client { common } => client_mode::run(common).await,
        Commands::Operator { common } => operator_mode::run(common).await,
    }
}
