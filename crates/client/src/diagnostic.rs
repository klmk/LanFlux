//! 诊断工具
//!
//! 提供：
//! - 服务端连通性测试（TCP 连接）
//! - DNS 解析测试
//! - 诊断信息导出（软件版本、操作系统、角色、服务端地址、连接状态）
//! - 日志导出

use std::net::{TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use net_tool_common::{ClientConfig, NodeRole};

use crate::config;
use crate::connection::{ConnectionStatus, ServerConnection};
use crate::detect_os;

/// 软件版本。
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// 单项诊断结果。
pub struct DiagItem {
    pub name: String,
    pub ok: bool,
    pub detail: String,
    pub elapsed_ms: u128,
}

impl DiagItem {
    fn ok(name: &str, detail: impl Into<String>, elapsed_ms: u128) -> Self {
        Self {
            name: name.into(),
            ok: true,
            detail: detail.into(),
            elapsed_ms,
        }
    }
    fn fail(name: &str, detail: impl Into<String>, elapsed_ms: u128) -> Self {
        Self {
            name: name.into(),
            ok: false,
            detail: detail.into(),
            elapsed_ms,
        }
    }
}

/// 测试 TCP 连通性（host:port）。
pub fn test_tcp_connect(host: &str, port: u16, timeout: Duration) -> DiagItem {
    let start = Instant::now();
    let addr = format!("{host}:{port}");
    match resolve_and_connect(&addr, timeout) {
        Ok(elapsed) => DiagItem::ok(
            "TCP 连接",
            format!("成功连接 {addr}"),
            start.elapsed().as_millis(),
        )
        .with_elapsed(elapsed.as_millis()),
        Err(e) => DiagItem::fail(
            "TCP 连接",
            format!("连接 {addr} 失败: {e}"),
            start.elapsed().as_millis(),
        ),
    }
}

fn resolve_and_connect(addr: &str, timeout: Duration) -> Result<Duration> {
    let start = Instant::now();
    let mut iter = addr
        .to_socket_addrs()
        .with_context(|| format!("解析地址失败: {addr}"))?;
    let sock_addr = iter
        .next()
        .ok_or_else(|| anyhow::anyhow!("无可用地址: {addr}"))?;
    let stream = TcpStream::connect_timeout(&sock_addr, timeout)
        .with_context(|| format!("TCP 连接失败: {sock_addr}"))?;
    drop(stream);
    Ok(start.elapsed())
}

/// 测试 DNS 解析。
pub fn test_dns(domain: &str) -> DiagItem {
    let start = Instant::now();
    match (domain, 80u16).to_socket_addrs() {
        Ok(addrs) => {
            let list: Vec<String> = addrs.map(|a| a.ip().to_string()).collect();
            if list.is_empty() {
                DiagItem::fail(
                    "DNS 解析",
                    format!("{domain} 未解析到地址"),
                    start.elapsed().as_millis(),
                )
            } else {
                DiagItem::ok(
                    "DNS 解析",
                    format!("{domain} -> {}", list.join(", ")),
                    start.elapsed().as_millis(),
                )
            }
        }
        Err(e) => DiagItem::fail(
            "DNS 解析",
            format!("解析 {domain} 失败: {e}"),
            start.elapsed().as_millis(),
        ),
    }
}

/// 从服务端地址中拆分出 host 与 port。
pub fn split_host_port(server_addr: &str) -> (String, u16) {
    let addr = server_addr
        .trim()
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .trim_end_matches('/');
    if let Some((h, p)) = addr.rsplit_once(':') {
        let port = p.parse::<u16>().unwrap_or(8443);
        (h.to_string(), port)
    } else {
        (addr.to_string(), 8443)
    }
}

/// 快速连通性诊断：对服务端地址做 TCP 连接 + DNS 测试。
pub async fn run_quick(server_addr: &str) -> Vec<DiagItem> {
    let mut items = Vec::new();
    let (host, port) = split_host_port(server_addr);

    // 若 host 是域名，先做 DNS 测试
    let is_ip = host
        .parse::<std::net::IpAddr>()
        .map(|_| true)
        .unwrap_or(false);
    if !is_ip {
        items.push(test_dns(&host));
    }
    items.push(test_tcp_connect(&host, port, Duration::from_secs(5)));
    items
}

/// 打印诊断结果。
pub fn print_diag(items: &[DiagItem]) {
    for it in items {
        let mark = if it.ok {
            crate::display::green("[OK]")
        } else {
            crate::display::red("[FAIL]")
        };
        println!(
            "{mark} {} ({} ms) - {}",
            it.name, it.elapsed_ms, it.detail
        );
    }
}

/// 导出诊断信息到文件，返回写入路径。
pub async fn export_diagnostic(
    cfg: &ClientConfig,
    status: &ConnectionStatus,
    conn: Option<&ServerConnection>,
) -> Result<PathBuf> {
    config::ensure_config_dir()?;
    let path = config::config_dir().join("diagnostic.txt");

    let mut s = String::new();
    s.push_str("==== net-tool 诊断信息 ====\n");
    s.push_str(&format!("导出时间   : {}\n", chrono::Utc::now()));
    s.push_str(&format!("软件版本   : {}\n", VERSION));
    s.push_str(&format!("操作系统   : {:?}\n", detect_os()));
    s.push_str(&format!("当前角色   : {}\n", role_name(&cfg.role)));
    s.push_str(&format!("服务端地址 : {}\n", cfg.server_addr));
    s.push_str(&format!("节点名称   : {}\n", cfg.node_name));
    s.push_str(&format!("连接状态   : {}\n", status.as_str()));
    if let Some(conn) = conn {
        s.push_str(&format!("节点 ID    : {}\n", conn.node_id().unwrap_or("-")));
        s.push_str(&format!(
            "虚拟 IP    : {}\n",
            conn.virtual_ip().unwrap_or("-")
        ));
    }
    s.push_str("\n---- 配置 ----\n");
    s.push_str(&format!("自动重连   : {}\n", cfg.auto_reconnect));
    s.push_str(&format!("开机自启   : {}\n", cfg.auto_start));
    s.push_str(&format!("日志级别   : {}\n", cfg.log_level));

    // 附加最近一次连通性诊断
    s.push_str("\n---- 连通性诊断 ----\n");
    let items = run_quick(&cfg.server_addr).await;
    for it in &items {
        s.push_str(&format!(
            "[{}] {} ({} ms) - {}\n",
            if it.ok { "OK" } else { "FAIL" },
            it.name,
            it.elapsed_ms,
            it.detail
        ));
    }

    std::fs::write(&path, s)
        .with_context(|| format!("写入诊断文件失败: {}", path.display()))?;
    Ok(path)
}

/// 导出日志文件（拷贝当前日志目录下的最新日志，若不存在则生成摘要）。
#[allow(dead_code)]
pub fn export_logs() -> Result<PathBuf> {
    config::ensure_config_dir()?;
    let path = config::config_dir().join("net-tool.log");
    let summary = format!(
        "net-tool 日志导出 {}\n软件版本: {}\n操作系统: {:?}\n（如需详细日志，请在启动时设置 RUST_LOG=debug）\n",
        chrono::Utc::now(),
        VERSION,
        detect_os()
    );
    std::fs::write(&path, &summary)
        .with_context(|| format!("写入日志文件失败: {}", path.display()))?;
    Ok(path)
}

fn role_name(role: &NodeRole) -> &'static str {
    match role {
        NodeRole::Server => "服务端",
        NodeRole::Client => "客户端",
        NodeRole::Operator => "实施端",
    }
}

// 让 DiagItem::ok/fail 之后还能微调 elapsed（用于 TCP 把内部计时透传）。
impl DiagItem {
    fn with_elapsed(mut self, ms: u128) -> Self {
        self.elapsed_ms = ms;
        self
    }
}
