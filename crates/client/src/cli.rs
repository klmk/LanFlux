//! CLI 参数定义与公共辅助函数
//!
//! 包含：
//! - `Cli` / `Commands` / `CommonArgs`：clap 命令行参数结构
//! - `detect_os()`：探测当前操作系统类型
//! - `read_line()` / `read_value()`：交互式输入辅助

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
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

/// 子命令
#[derive(Subcommand)]
pub enum Commands {
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
pub struct CommonArgs {
    /// 服务端地址，如 127.0.0.1:8443 或 https://server.example.com:8443
    #[arg(long, env = "NET_TOOL_SERVER_ADDR")]
    pub server_addr: Option<String>,

    /// 节点名称
    #[arg(long)]
    pub name: Option<String>,

    /// 备注
    #[arg(long)]
    pub remark: Option<String>,

    /// 是否启用自动重连（不传则使用配置文件；传 `--auto-reconnect` 等价于 true，
    /// 传 `--auto-reconnect false` 关闭）
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    pub auto_reconnect: Option<bool>,

    /// 是否开机自启
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    pub auto_start: Option<bool>,

    /// 配置文件路径（默认 ~/.net-tool/config.toml）
    #[arg(long)]
    pub config: Option<String>,

    /// 启动前先运行一次连通性诊断
    #[arg(long, default_value_t = false)]
    pub diagnostic: bool,

    /// 日志级别：error / warn / info / debug / trace
    #[arg(long)]
    pub log_level: Option<String>,
}

impl Cli {
    /// 取得任意子命令携带的日志级别（用于在模式分发前初始化日志）。
    pub fn common_log_level(&self) -> Option<String> {
        match &self.command {
            Commands::Server { common, .. }
            | Commands::Client { common }
            | Commands::Operator { common } => common.log_level.clone(),
        }
    }
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

/// 交互式读取一行输入（带提示符）。
pub async fn read_line(prompt: &str) -> Result<String> {
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
pub async fn read_value<T: std::str::FromStr>(prompt: &str) -> Result<T>
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

/// 初始化日志订阅器。
pub fn init_tracing(log_level: &str) {
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(log_level));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init()
        .ok();
}
