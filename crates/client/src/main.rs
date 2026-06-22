//! net-tool 客户端 / 实施端 CLI 入口
//!
//! 同一个二进制支持三种运行模式：
//! - `server`：以服务端模式运行（轻量内嵌，生产环境建议使用独立服务端）
//! - `client`：以普通客户端模式运行，上报真实网段
//! - `operator`：以实施端模式运行，访问客户端网段
//!
//! 使用 [`clap`] 解析命令行参数，使用 [`tokio`] 作为异步运行时。

mod client_mode;
mod config;
mod connection;
mod diagnostic;
mod display;
mod operator_mode;
mod scanner;
mod server_mode;
mod tunnel_client;

use anyhow::Result;
use clap::{Parser, Subcommand};
use net_tool_common::OsType;

/// 命令行顶层结构
#[derive(Parser)]
#[command(
    name = "net-tool",
    version,
    about = "组网工具 - 同一二进制支持 server / client / operator 三种模式"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

/// 子命令
#[derive(Subcommand)]
enum Commands {
    /// 以服务端模式运行（轻量内嵌，生产环境建议使用 Linux / Docker 部署独立服务端）
    Server {
        /// 监听地址，如 0.0.0.0:8443
        #[arg(long, default_value = "0.0.0.0:8443")]
        bind: String,

        /// 启动后自动打开 Web 后台
        #[arg(long, default_value_t = false)]
        open: bool,

        #[command(flatten)]
        common: CommonArgs,
    },
    /// 以普通客户端模式运行（上报真实网段）
    Client {
        #[command(flatten)]
        common: CommonArgs,
    },
    /// 以实施端模式运行（访问客户端网段）
    Operator {
        #[command(flatten)]
        common: CommonArgs,
    },
}

/// 各子命令共享的公共参数
#[derive(Parser, Clone, Debug)]
struct CommonArgs {
    /// 服务端地址，如 127.0.0.1:8443 或 https://server.example.com:8443
    #[arg(long, env = "NET_TOOL_SERVER_ADDR")]
    server_addr: Option<String>,

    /// 节点名称
    #[arg(long)]
    name: Option<String>,

    /// 备注
    #[arg(long)]
    remark: Option<String>,

    /// 是否启用自动重连（不传则使用配置文件；传 `--auto-reconnect` 等价于 true，
    /// 传 `--auto-reconnect false` 关闭）
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    auto_reconnect: Option<bool>,

    /// 是否开机自启
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    auto_start: Option<bool>,

    /// 配置文件路径（默认 ~/.net-tool/config.toml）
    #[arg(long)]
    config: Option<String>,

    /// 启动前先运行一次连通性诊断
    #[arg(long, default_value_t = false)]
    diagnostic: bool,

    /// 日志级别：error / warn / info / debug / trace
    #[arg(long)]
    log_level: Option<String>,
}

/// 探测当前操作系统类型。
pub fn detect_os() -> OsType {
    #[cfg(target_os = "linux")]
    {
        OsType::Linux
    }
    #[cfg(target_os = "windows")]
    {
        OsType::Windows
    }
    #[cfg(target_os = "android")]
    {
        OsType::Android
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "android")))]
    {
        OsType::Unknown
    }
}

/// 初始化日志订阅器。
fn init_tracing(log_level: &str) {
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(log_level));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init()
        .ok();
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // 先用一个默认级别初始化日志，各模式内部会按配置覆盖。
    let default_level = cli
        .common_log_level()
        .unwrap_or_else(|| "info".to_string());
    init_tracing(&default_level);

    match cli.command {
        Commands::Server { bind, open, common } => {
            server_mode::run(bind, open, common).await
        }
        Commands::Client { common } => client_mode::run(common).await,
        Commands::Operator { common } => operator_mode::run(common).await,
    }
}

impl Cli {
    /// 取得任意子命令携带的日志级别（用于在模式分发前初始化日志）。
    fn common_log_level(&self) -> Option<String> {
        match &self.command {
            Commands::Server { common, .. }
            | Commands::Client { common }
            | Commands::Operator { common } => common.log_level.clone(),
        }
    }
}

/// 交互式读取一行输入（带提示符）。
pub(crate) async fn read_line(prompt: &str) -> Result<String> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let mut stdout = tokio::io::stdout();
    print!("{prompt}");
    stdout.flush().await?;
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);
    let mut line = String::new();
    if reader.read_line(&mut line).await? == 0 {
        return Ok(String::new());
    }
    Ok(line.trim().to_string())
}

/// 读取一行并解析为指定类型，失败时提示重新输入。
#[allow(dead_code)]
pub(crate) async fn read_value<T: std::str::FromStr>(prompt: &str) -> Result<T>
where
    <T as std::str::FromStr>::Err: std::fmt::Display,
{
    loop {
        let line = read_line(prompt).await?;
        match line.parse::<T>() {
            Ok(v) => return Ok(v),
            Err(e) => {
                println!("输入无效: {e}，请重试。");
            }
        }
    }
}
