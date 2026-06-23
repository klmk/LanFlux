//! Tauri 命令实现
//!
//! 提供前端可调用的 Rust 后端命令：
//! - `scan_interfaces`：扫描本机网卡，返回网卡列表
//! - `get_app_info`：返回应用版本与操作系统信息
//! - `test_connectivity`：测试服务端连通性（TCP 连接）
//! - `save_config`：保存配置到本地文件
//! - `load_config`：从本地文件加载配置

use std::collections::HashMap;
use std::fs;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::client_manager::{
    ClientManager, ConnectResult, ConvertIpResult, PingResult, RouteInfo, StatusInfo,
    TcpTestResult, TunnelStartResult,
};

// ============================================================
// 数据结构
// ============================================================

/// 网卡类型字符串
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterfaceInfo {
    /// 网卡名称
    pub name: String,
    /// 网卡类型（中文描述）
    pub iface_type: String,
    /// IPv4 地址
    pub ip_address: String,
    /// 推测网段，如 `192.168.1.0/24`
    pub cidr: String,
    /// 网关
    pub gateway: Option<String>,
    /// 是否推荐上报
    pub recommended: bool,
}

/// 应用信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppInfo {
    /// 应用名称
    pub name: String,
    /// 版本号
    pub version: String,
    /// 操作系统
    pub os: String,
    /// 系统架构
    pub arch: String,
}

/// 连通性测试结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectivityResult {
    /// 是否成功
    pub success: bool,
    /// 耗时（毫秒）
    pub elapsed_ms: u64,
    /// 详细信息
    pub message: String,
}

/// 桌面端配置（保存到本地文件）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopConfig {
    /// 当前模式：server / client / operator
    pub mode: String,
    /// 服务端地址
    pub server_addr: String,
    /// 节点名称
    pub node_name: String,
    /// 备注
    pub remark: String,
    /// 是否自动重连
    pub auto_reconnect: bool,
}

impl Default for DesktopConfig {
    fn default() -> Self {
        Self {
            mode: String::new(),
            server_addr: "127.0.0.1:8443".to_string(),
            node_name: String::new(),
            remark: String::new(),
            auto_reconnect: true,
        }
    }
}

// ============================================================
// 网卡类型枚举（内部使用）
// ============================================================

#[derive(Debug, Clone, PartialEq, Eq)]
enum InterfaceType {
    Physical,
    Wifi,
    Vmware,
    HyperV,
    Docker,
    Vpn,
    Loopback,
    Unknown,
}

impl InterfaceType {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Physical => "物理网卡",
            Self::Wifi => "Wi-Fi",
            Self::Vmware => "VMware",
            Self::HyperV => "Hyper-V",
            Self::Docker => "Docker",
            Self::Vpn => "VPN",
            Self::Loopback => "Loopback",
            Self::Unknown => "未知",
        }
    }

    fn is_recommended(&self) -> bool {
        matches!(self, Self::Physical | Self::Wifi)
    }
}

// ============================================================
// 命令实现
// ============================================================

/// 扫描本机网卡，返回候选网段列表（已排除 Loopback）。
#[tauri::command]
pub fn scan_interfaces() -> Vec<NetworkInterfaceInfo> {
    match scan_platform() {
        Ok(mut list) => {
            // 排除 Loopback
            list.retain(|(iface_type, _)| *iface_type != InterfaceType::Loopback);
            // 排除既无 IP 也无网段的接口
            list.retain(|(_, info)| !info.ip_address.is_empty() || !info.cidr.is_empty());

            list.into_iter()
                .map(|(iface_type, info)| NetworkInterfaceInfo {
                    name: info.name,
                    iface_type: iface_type.as_str().to_string(),
                    ip_address: info.ip_address,
                    cidr: info.cidr,
                    gateway: info.gateway,
                    recommended: iface_type.is_recommended(),
                })
                .collect()
        }
        Err(_) => Vec::new(),
    }
}

/// 返回应用版本与操作系统信息。
#[tauri::command]
pub fn get_app_info() -> AppInfo {
    AppInfo {
        name: "NetTool 组网工具".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
    }
}

/// 测试服务端连通性（TCP 连接）。
#[tauri::command]
pub fn test_connectivity(host: String, port: u16) -> ConnectivityResult {
    let start = Instant::now();
    let addr = format!("{host}:{port}");

    match addr.to_socket_addrs() {
        Ok(mut iter) => {
            if let Some(sock_addr) = iter.next() {
                match TcpStream::connect_timeout(&sock_addr, Duration::from_secs(5)) {
                    Ok(_) => ConnectivityResult {
                        success: true,
                        elapsed_ms: start.elapsed().as_millis() as u64,
                        message: format!("成功连接 {addr}"),
                    },
                    Err(e) => ConnectivityResult {
                        success: false,
                        elapsed_ms: start.elapsed().as_millis() as u64,
                        message: format!("连接 {addr} 失败: {e}"),
                    },
                }
            } else {
                ConnectivityResult {
                    success: false,
                    elapsed_ms: start.elapsed().as_millis() as u64,
                    message: format!("无可用地址: {addr}"),
                }
            }
        }
        Err(e) => ConnectivityResult {
            success: false,
            elapsed_ms: start.elapsed().as_millis() as u64,
            message: format!("解析地址失败: {addr} - {e}"),
        },
    }
}

/// 保存配置到本地文件。
#[tauri::command]
pub fn save_config(config: DesktopConfig) -> Result<String, String> {
    let path = config_path()?;
    let json = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| format!("写入配置失败: {}", e))?;
    Ok(path.to_string_lossy().to_string())
}

/// 从本地文件加载配置。
#[tauri::command]
pub fn load_config() -> Result<DesktopConfig, String> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(DesktopConfig::default());
    }
    let content = fs::read_to_string(&path).map_err(|e| format!("读取配置失败: {}", e))?;
    let config: DesktopConfig = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    Ok(config)
}

// ============================================================
// 网络连接命令
// ============================================================

/// 以客户端模式连接服务端
#[tauri::command]
pub async fn connect_client(
    manager: tauri::State<'_, ClientManager>,
    server_addr: String,
    node_name: String,
    remark: Option<String>,
) -> Result<ConnectResult, String> {
    manager
        .connect_client(&server_addr, &node_name, remark)
        .await
}

/// 以实施端模式连接服务端
#[tauri::command]
pub async fn connect_operator(
    manager: tauri::State<'_, ClientManager>,
    server_addr: String,
    node_name: String,
) -> Result<ConnectResult, String> {
    manager.connect_operator(&server_addr, &node_name).await
}

/// 启动隧道
#[tauri::command]
pub async fn start_tunnel(
    manager: tauri::State<'_, ClientManager>,
) -> Result<TunnelStartResult, String> {
    manager.start_tunnel().await
}

/// 停止隧道
#[tauri::command]
pub async fn stop_tunnel(
    manager: tauri::State<'_, ClientManager>,
) -> Result<(), String> {
    manager.stop_tunnel().await
}

/// 断开所有连接
#[tauri::command]
pub async fn disconnect_all(
    manager: tauri::State<'_, ClientManager>,
) -> Result<(), String> {
    manager.disconnect().await
}

/// 获取综合状态
#[tauri::command]
pub async fn get_status(
    manager: tauri::State<'_, ClientManager>,
) -> Result<StatusInfo, String> {
    Ok(manager.get_status().await)
}

/// 获取隧道路由
#[tauri::command]
pub async fn get_tunnel_routes(
    manager: tauri::State<'_, ClientManager>,
) -> Result<Vec<RouteInfo>, String> {
    Ok(manager.get_tunnel_routes().await)
}

/// 获取可访问网段（实施端）
#[tauri::command]
pub async fn get_accessible_segments(
    manager: tauri::State<'_, ClientManager>,
) -> Result<Vec<net_tool_common::RouteEntry>, String> {
    Ok(manager.get_accessible_segments().await)
}

/// 刷新可访问网段
#[tauri::command]
pub async fn refresh_accessible_segments(
    manager: tauri::State<'_, ClientManager>,
) -> Result<Vec<net_tool_common::RouteEntry>, String> {
    manager.refresh_accessible_segments().await
}

/// 获取已上报网段（客户端）
#[tauri::command]
pub async fn get_reported_segments(
    manager: tauri::State<'_, ClientManager>,
) -> Result<Vec<net_tool_common::SegmentSummary>, String> {
    Ok(manager.get_reported_segments().await)
}

/// 刷新已上报网段
#[tauri::command]
pub async fn refresh_reported_segments(
    manager: tauri::State<'_, ClientManager>,
) -> Result<Vec<net_tool_common::SegmentSummary>, String> {
    manager.refresh_reported_segments().await
}

/// 上报网段
#[tauri::command]
pub async fn report_segment(
    manager: tauri::State<'_, ClientManager>,
    name: String,
    real_cidr: String,
    remark: Option<String>,
) -> Result<net_tool_common::ReportSegmentResponse, String> {
    manager.report_segment(&name, &real_cidr, remark).await
}

/// Ping 测试
#[tauri::command]
pub async fn ping_test(
    manager: tauri::State<'_, ClientManager>,
    target_ip: String,
) -> Result<PingResult, String> {
    Ok(manager.ping_test(&target_ip).await)
}

/// TCP 连通性测试
#[tauri::command]
pub async fn tcp_test(
    manager: tauri::State<'_, ClientManager>,
    target_ip: String,
    port: u16,
) -> Result<TcpTestResult, String> {
    Ok(manager.tcp_test(&target_ip, port).await)
}

/// IP 转换
#[tauri::command]
pub async fn convert_ip(
    manager: tauri::State<'_, ClientManager>,
    input_ip: String,
) -> Result<ConvertIpResult, String> {
    Ok(manager.convert_ip(&input_ip).await)
}

// ============================================================
// 配置文件路径
// ============================================================

fn config_dir() -> Result<PathBuf, String> {
    let dir = dirs_config_dir()?;
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    Ok(dir)
}

fn config_path() -> Result<PathBuf, String> {
    Ok(config_dir()?.join("config.json"))
}

#[cfg(target_os = "windows")]
fn dirs_config_dir() -> Result<PathBuf, String> {
    let home = std::env::var("APPDATA").map_err(|_| "无法获取 APPDATA".to_string())?;
    Ok(PathBuf::from(home).join("net-tool"))
}

#[cfg(not(target_os = "windows"))]
fn dirs_config_dir() -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|_| "无法获取 HOME".to_string())?;
    Ok(PathBuf::from(home).join(".config").join("net-tool"))
}

// ============================================================
// 平台相关网卡扫描
// ============================================================

/// 内部使用的网卡扫描结果
struct RawInterface {
    name: String,
    ip_address: String,
    cidr: String,
    gateway: Option<String>,
}

#[cfg(target_os = "linux")]
fn scan_platform() -> Result<Vec<(InterfaceType, RawInterface)>, Box<dyn std::error::Error>> {
    use std::path::Path;

    let names = read_interface_names()?;
    let routes = read_routes()?;
    let local_ips = read_local_ips();

    // 按 iface 聚合路由
    let mut by_iface: HashMap<String, Vec<RouteLine>> = HashMap::new();
    for r in routes {
        by_iface.entry(r.iface.clone()).or_default().push(r);
    }

    let mut out = Vec::new();
    for name in &names {
        let iface_type = detect_type(name);

        let routes = by_iface.get(name).cloned().unwrap_or_default();
        // 子网路由：掩码非 0 且非 /32 中前缀最大者
        let subnet = routes
            .iter()
            .filter(|r| r.mask != 0 && r.mask != 0xFFFF_FFFF)
            .max_by_key(|r| r.mask);
        // 默认网关：dest == 0
        let default_gw = routes.iter().find(|r| r.dest == 0 && r.mask == 0);

        let (network, prefix) = match subnet {
            Some(r) => (
                r.dest,
                net_tool_network::prefix_from_netmask(r.mask).unwrap_or(24),
            ),
            None => (0, 0),
        };

        // 从本地地址中找到属于该子网的主机 IP
        let ip_address = local_ips
            .iter()
            .find(|(ip, _)| net_tool_network::ip_in_network(*ip, network, prefix))
            .map(|(ip, _)| net_tool_network::u32_to_ip(*ip))
            .unwrap_or_default();

        let cidr = if prefix > 0 {
            format!("{}/{}", net_tool_network::u32_to_ip(network), prefix)
        } else {
            String::new()
        };

        let gateway = default_gw
            .filter(|r| r.gateway != 0)
            .map(|r| net_tool_network::u32_to_ip(r.gateway));

        out.push((
            iface_type,
            RawInterface {
                name: name.clone(),
                ip_address,
                cidr,
                gateway,
            },
        ));
    }
    Ok(out)
}

#[cfg(target_os = "linux")]
struct RouteLine {
    iface: String,
    dest: u32,
    gateway: u32,
    mask: u32,
}

#[cfg(target_os = "linux")]
fn read_interface_names() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let content = fs::read_to_string("/proc/net/dev")?;
    let mut names = Vec::new();
    for line in content.lines().skip(2) {
        if let Some(name) = line.split(':').next() {
            let name = name.trim().to_string();
            if !name.is_empty() {
                names.push(name);
            }
        }
    }
    Ok(names)
}

#[cfg(target_os = "linux")]
fn read_routes() -> Result<Vec<RouteLine>, Box<dyn std::error::Error>> {
    let content = fs::read_to_string("/proc/net/route")?;
    let mut out = Vec::new();
    for (i, line) in content.lines().enumerate() {
        if i == 0 {
            continue;
        }
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 8 {
            continue;
        }
        let iface = fields[0].to_string();
        let dest = parse_hex_le(fields[1]);
        let gateway = parse_hex_le(fields[2]);
        let mask = parse_hex_le(fields[7]);
        if let (Some(dest), Some(gateway), Some(mask)) = (dest, gateway, mask) {
            out.push(RouteLine {
                iface,
                dest,
                gateway,
                mask,
            });
        }
    }
    Ok(out)
}

#[cfg(target_os = "linux")]
fn read_local_ips() -> Vec<(u32, u8)> {
    let content = match fs::read_to_string("/proc/net/fib_trie") {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let lines: Vec<&str> = content.lines().collect();
    let mut out = Vec::new();
    for i in 0..lines.len() {
        if lines[i].contains("host LOCAL") {
            if i > 0 {
                if let Some(ip) = first_ipv4(lines[i - 1]) {
                    out.push((ip, 32));
                }
            }
        }
    }
    out
}

#[cfg(target_os = "linux")]
fn parse_hex_le(s: &str) -> Option<u32> {
    let v = u32::from_str_radix(s, 16).ok()?;
    Some(u32::from_le(v))
}

#[cfg(target_os = "linux")]
fn first_ipv4(s: &str) -> Option<u32> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    for p in parts {
        let candidate = p.trim_matches(|c: char| !c.is_ascii_digit() && c != '.');
        if let Ok(ip) = net_tool_network::ip_to_u32(candidate) {
            return Some(ip);
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn detect_type(name: &str) -> InterfaceType {
    use std::path::Path;
    if name == "lo" {
        return InterfaceType::Loopback;
    }
    let base = format!("/sys/class/net/{name}");
    if Path::new(&format!("{base}/wireless")).exists() {
        return InterfaceType::Wifi;
    }
    let lower = name.to_lowercase();
    if lower.starts_with("docker") || lower.starts_with("br-") || lower.starts_with("veth") {
        return InterfaceType::Docker;
    }
    if lower.starts_with("vmnet") {
        return InterfaceType::Vmware;
    }
    if lower.starts_with("vernet") {
        return InterfaceType::HyperV;
    }
    if lower.starts_with("tun")
        || lower.starts_with("tap")
        || lower.starts_with("ppp")
        || lower.starts_with("wg")
    {
        return InterfaceType::Vpn;
    }
    if let Some(driver) = read_driver(name) {
        let d = driver.to_lowercase();
        if d.contains("hv_netvsc") {
            return InterfaceType::HyperV;
        }
        if d.contains("vmxnet") || d.contains("vmware") {
            return InterfaceType::Vmware;
        }
    }
    if Path::new(&format!("{base}/device")).exists() {
        return InterfaceType::Physical;
    }
    InterfaceType::Unknown
}

#[cfg(target_os = "linux")]
fn read_driver(iface: &str) -> Option<String> {
    let p = format!("/sys/class/net/{iface}/device/driver");
    fs::read_link(&p)
        .ok()
        .and_then(|p| p.file_name().map(|s| s.to_string_lossy().to_string()))
}

// ============================================================
// Windows 实现（解析 ipconfig 输出）
// ============================================================

#[cfg(target_os = "windows")]
fn scan_platform() -> Result<Vec<(InterfaceType, RawInterface)>, Box<dyn std::error::Error>> {
    use std::process::Command;

    let output = Command::new("ipconfig").args(["/all"]).output()?;
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    parse_ipconfig(&text)
}

#[cfg(target_os = "windows")]
fn parse_ipconfig(text: &str) -> Result<Vec<(InterfaceType, RawInterface)>, Box<dyn std::error::Error>> {
    let mut out = Vec::new();
    let mut current_name: Option<String> = None;
    let mut ip: Option<String> = None;
    let mut mask: Option<String> = None;
    let mut gateway: Option<String> = None;

    let flush = |out: &mut Vec<(InterfaceType, RawInterface)>,
                 name: &Option<String>,
                 ip: &Option<String>,
                 mask: &Option<String>,
                 gateway: &Option<String>| {
        if let (Some(name), Some(ip)) = (name, ip) {
            let iface_type = detect_type(name);
            let cidr = match mask {
                Some(m) => net_tool_network::cidr_from_ip_mask(ip, m).unwrap_or_default(),
                None => String::new(),
            };
            out.push((
                iface_type,
                RawInterface {
                    name: name.clone(),
                    ip_address: ip.clone(),
                    cidr,
                    gateway: gateway.clone(),
                },
            ));
        }
    };

    for raw in text.lines() {
        let line = raw.trim();
        if (line.contains("adapter") || line.contains("适配器")) && line.ends_with(':') {
            flush(&mut out, &current_name, &ip, &mask, &gateway);
            current_name = Some(line.trim_end_matches(':').to_string());
            ip = None;
            mask = None;
            gateway = None;
            continue;
        }
        let lower = line.to_lowercase();
        if lower.contains("ipv4") || line.contains("IPv4 地址") {
            if let Some(v) = split_value(line) {
                ip = Some(v);
            }
        } else if lower.contains("subnet mask") || line.contains("子网掩码") {
            if let Some(v) = split_value(line) {
                mask = Some(v);
            }
        } else if lower.contains("default gateway") || line.contains("默认网关") {
            if let Some(v) = split_value(line) {
                gateway = Some(v);
            }
        }
    }
    flush(&mut out, &current_name, &ip, &mask, &gateway);
    Ok(out)
}

#[cfg(target_os = "windows")]
fn split_value(line: &str) -> Option<String> {
    let v = line.split(':').nth(1)?.trim();
    if v.is_empty() {
        None
    } else {
        Some(v.trim_matches(|c: char| c == '(' || c == ')').to_string())
    }
}

#[cfg(target_os = "windows")]
fn detect_type(name: &str) -> InterfaceType {
    let lower = name.to_lowercase();
    if lower.contains("loopback") || lower.contains("环回") {
        return InterfaceType::Loopback;
    }
    if lower.contains("wireless") || lower.contains("wi-fi") || lower.contains("无线") {
        return InterfaceType::Wifi;
    }
    if lower.contains("vmware") || lower.contains("vmnet") {
        return InterfaceType::Vmware;
    }
    if lower.contains("hyper-v") || lower.contains("vernet") {
        return InterfaceType::HyperV;
    }
    if lower.contains("docker") || lower.contains("vethernet") {
        return InterfaceType::Docker;
    }
    if lower.contains("vpn") || lower.contains("tun") || lower.contains("tap") {
        return InterfaceType::Vpn;
    }
    InterfaceType::Physical
}

// ============================================================
// 其它平台占位实现
// ============================================================

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn scan_platform() -> Result<Vec<(InterfaceType, RawInterface)>, Box<dyn std::error::Error>> {
    Ok(Vec::new())
}
