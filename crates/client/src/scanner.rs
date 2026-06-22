//! 本机网卡扫描
//!
//! 扫描本机所有网络接口，识别网卡类型并推测网段（CIDR）。
//!
//! 由于不使用外部系统命令（Windows 的 `ipconfig` 除外），采用条件编译：
//! - Linux：读取 `/proc/net/dev`、`/sys/class/net/`、`/proc/net/route`、
//!   `/proc/net/fib_trie`
//! - Windows：解析 `ipconfig` 输出（通过 [`std::process::Command`]）
//! - 其它平台：返回空列表
//!
//! 推荐规则：物理网卡与 Wi-Fi 标记为推荐；虚拟网卡标记为可选；Loopback 排除。

use anyhow::Result;

/// 网卡类型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InterfaceType {
    /// 物理网卡
    Physical,
    /// Wi-Fi
    Wifi,
    /// VMware 虚拟网卡
    Vmware,
    /// Hyper-V 虚拟网卡
    HyperV,
    /// Docker 虚拟网卡
    Docker,
    /// VPN / 隧道网卡
    Vpn,
    /// 回环
    Loopback,
    /// 未知
    Unknown,
}

impl InterfaceType {
    pub fn as_str(&self) -> &'static str {
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

    #[allow(dead_code)]
    pub fn is_virtual(&self) -> bool {
        matches!(
            self,
            Self::Vmware | Self::HyperV | Self::Docker | Self::Vpn | Self::Unknown
        )
    }
}

/// 一张网卡的信息。
#[derive(Debug, Clone)]
pub struct NetworkInterface {
    /// 网卡名称
    pub name: String,
    /// 网卡类型
    pub iface_type: InterfaceType,
    /// IPv4 地址
    pub ip_address: String,
    /// 推测网段，如 `192.168.1.0/24`
    pub cidr: String,
    /// 网关
    pub gateway: Option<String>,
    /// 是否推荐上报
    pub recommended: bool,
}

/// 扫描本机网卡，返回候选网段列表（已排除 Loopback）。
pub fn scan_interfaces() -> Result<Vec<NetworkInterface>> {
    let mut interfaces = scan_platform()?;
    // 排除 Loopback
    interfaces.retain(|i| i.iface_type != InterfaceType::Loopback);
    // 排除既无 IP 也无网段的接口
    interfaces.retain(|i| !i.ip_address.is_empty() || !i.cidr.is_empty());
    // 推荐标记
    for iface in &mut interfaces {
        iface.recommended =
            matches!(iface.iface_type, InterfaceType::Physical | InterfaceType::Wifi);
    }
    Ok(interfaces)
}

// ============================================================================
// Linux 实现
// ============================================================================
#[cfg(target_os = "linux")]
mod platform {
    use std::collections::HashMap;
    use std::fs;
    use std::path::Path;

    use anyhow::{Context, Result};

    use super::{InterfaceType, NetworkInterface};
    use net_tool_network as net;

    /// 一行 /proc/net/route 解析结果（值已转为主机字节序的 u32）。
    #[derive(Clone)]
    struct RouteLine {
        iface: String,
        dest: u32,
        gateway: u32,
        mask: u32,
    }

    pub fn scan_platform() -> Result<Vec<NetworkInterface>> {
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
            if iface_type == InterfaceType::Loopback {
                // 仍加入列表，由上层 scan_interfaces 统一排除
            }

            let routes = by_iface.get(name).cloned().unwrap_or_default();
            // 子网路由：掩码非 0 且非 /32 中前缀最大者
            let subnet = routes
                .iter()
                .filter(|r| r.mask != 0 && r.mask != 0xFFFF_FFFF)
                .max_by_key(|r| r.mask);
            // 默认网关：dest == 0
            let default_gw = routes.iter().find(|r| r.dest == 0 && r.mask == 0);

            let (network, prefix) = match subnet {
                Some(r) => (r.dest, net::prefix_from_netmask(r.mask).unwrap_or(24)),
                None => (0, 0),
            };

            // 从 fib_trie 本地地址中找到属于该子网的主机 IP
            let ip_address = local_ips
                .iter()
                .find(|(ip, _)| net::ip_in_network(*ip, network, prefix))
                .map(|(ip, _)| net::u32_to_ip(*ip))
                .unwrap_or_default();

            let cidr = if prefix > 0 {
                format!("{}/{}", net::u32_to_ip(network), prefix)
            } else {
                String::new()
            };

            let gateway = default_gw
                .filter(|r| r.gateway != 0)
                .map(|r| net::u32_to_ip(r.gateway));

            out.push(NetworkInterface {
                name: name.clone(),
                iface_type,
                ip_address,
                cidr,
                gateway,
                recommended: false,
            });
        }
        Ok(out)
    }

    /// 读取 /proc/net/dev 中的网卡名列表。
    fn read_interface_names() -> Result<Vec<String>> {
        let content =
            fs::read_to_string("/proc/net/dev").context("读取 /proc/net/dev 失败")?;
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

    /// 解析 /proc/net/route。
    fn read_routes() -> Result<Vec<RouteLine>> {
        let content =
            fs::read_to_string("/proc/net/route").context("读取 /proc/net/route 失败")?;
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

    /// 解析 /proc/net/fib_trie 中的本地 /32 地址。
    ///
    /// 只收集前缀为 /32 的主机地址，避免把网络地址（如 192.168.1.0/24）
    /// 误当作主机 IP。
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
                        let prefix = first_prefix(lines[i]).unwrap_or(32);
                        // 仅保留 /32 主机地址，排除网络地址条目
                        if prefix == 32 {
                            out.push((ip, prefix));
                        }
                    }
                }
            }
        }
        out
    }

    /// /proc/net/route 中的值为小端十六进制，转为主机字节序 u32。
    fn parse_hex_le(s: &str) -> Option<u32> {
        let v = u32::from_str_radix(s, 16).ok()?;
        Some(u32::from_le(v))
    }

    /// 从字符串中提取第一个 IPv4 并转为 u32。
    fn first_ipv4(s: &str) -> Option<u32> {
        let parts: Vec<&str> = s.split_whitespace().collect();
        for p in parts {
            let candidate = p.trim_matches(|c: char| !c.is_ascii_digit() && c != '.');
            if let Ok(ip) = net::ip_to_u32(candidate) {
                return Some(ip);
            }
        }
        None
    }

    /// 从字符串中提取 `/N` 前缀长度。
    fn first_prefix(s: &str) -> Option<u8> {
        for tok in s.split_whitespace() {
            if let Some(rest) = tok.strip_prefix('/') {
                if let Ok(n) = rest.parse::<u8>() {
                    return Some(n);
                }
            }
        }
        None
    }

    /// 根据 /sys/class/net 与命名规则识别网卡类型。
    fn detect_type(name: &str) -> InterfaceType {
        if name == "lo" {
            return InterfaceType::Loopback;
        }
        let base = format!("/sys/class/net/{name}");
        // Wi-Fi：存在 wireless 目录
        if Path::new(&format!("{base}/wireless")).exists() {
            return InterfaceType::Wifi;
        }
        // 命名规则
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
        // 驱动名识别 Hyper-V / VMware
        if let Some(driver) = read_driver(name) {
            let d = driver.to_lowercase();
            if d.contains("hv_netvsc") {
                return InterfaceType::HyperV;
            }
            if d.contains("vmxnet") || d.contains("vmware") {
                return InterfaceType::Vmware;
            }
        }
        // 存在 device 链接 -> 真实物理网卡
        if Path::new(&format!("{base}/device")).exists() {
            return InterfaceType::Physical;
        }
        InterfaceType::Unknown
    }

    /// 读取 /sys/class/net/<iface>/device/driver 软链接指向的驱动名。
    fn read_driver(iface: &str) -> Option<String> {
        let p = format!("/sys/class/net/{iface}/device/driver");
        fs::read_link(&p)
            .ok()
            .and_then(|p| p.file_name().map(|s| s.to_string_lossy().to_string()))
    }
}

// ============================================================================
// Windows 实现（解析 ipconfig 输出）
// ============================================================================
#[cfg(target_os = "windows")]
mod platform {
    use std::process::Command;

    use anyhow::{Context, Result};

    use super::{InterfaceType, NetworkInterface};
    use net_tool_network as net;

    pub fn scan_platform() -> Result<Vec<NetworkInterface>> {
        let output = Command::new("ipconfig")
            .args(["/all"])
            .output()
            .context("执行 ipconfig 失败")?;
        // ipconfig 在中文 Windows 上可能使用 GBK 编码，这里做尽力解码。
        let text = decode_output(&output.stdout);
        parse_ipconfig(&text)
    }

    fn decode_output(bytes: &[u8]) -> String {
        // 优先 UTF-8，失败则用 GBK 近似（lossy）。
        if let Ok(s) = std::str::from_utf8(bytes) {
            return s.to_string();
        }
        // 简易 GBK->UTF8 不可得（无外部 crate），退回 lossy。
        String::from_utf8_lossy(bytes).to_string()
    }

    fn parse_ipconfig(text: &str) -> Result<Vec<NetworkInterface>> {
        let mut out = Vec::new();
        let mut current_name: Option<String> = None;
        let mut ip: Option<String> = None;
        let mut mask: Option<String> = None;
        let mut gateway: Option<String> = None;

        let flush = |out: &mut Vec<NetworkInterface>,
                     name: &Option<String>,
                     ip: &Option<String>,
                     mask: &Option<String>,
                     gateway: &Option<String>| {
            if let (Some(name), Some(ip)) = (name, ip) {
                let iface_type = detect_type(name);
                let cidr = match (ip, mask) {
                    (ip, Some(m)) => net::cidr_from_ip_mask(ip, m).unwrap_or_default(),
                    _ => String::new(),
                };
                out.push(NetworkInterface {
                    name: name.clone(),
                    iface_type,
                    ip_address: ip.clone(),
                    cidr,
                    gateway: gateway.clone(),
                    recommended: false,
                });
            }
        };

        for raw in text.lines() {
            let line = raw.trim();
            // 适配器标题行形如 "以太网适配器 以太网:" 或 "Wireless LAN adapter Wi-Fi:"
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

    /// 取 `xxx . . . : value` 中冒号后的值。
    fn split_value(line: &str) -> Option<String> {
        let v = line.split(':').nth(1)?.trim();
        if v.is_empty() {
            None
        } else {
            // 去掉可能的括号首选标记
            Some(v.trim_matches(|c: char| c == '(' || c == ')').to_string())
        }
    }

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
        if lower.contains("docker") || lower.contains("vethernet") && lower.contains("ws") {
            return InterfaceType::Docker;
        }
        if lower.contains("vpn") || lower.contains("tun") || lower.contains("tap") {
            return InterfaceType::Vpn;
        }
        InterfaceType::Physical
    }
}

// ============================================================================
// 其它平台占位实现
// ============================================================================
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
mod platform {
    use anyhow::Result;

    use super::NetworkInterface;

    pub fn scan_platform() -> Result<Vec<NetworkInterface>> {
        Ok(Vec::new())
    }
}

/// 平台相关扫描入口。
pub use platform::scan_platform;
