//! IPv4 / CIDR 换算工具
//!
//! 本模块不依赖任何外部网络系统调用，仅做纯数值换算。
//! 所有 IP 在内部以 `u32`（主机字节序，最高位为第一字节）表示，
//! 例如 `192.168.1.5` 对应 `0xC0A80105`。

use std::net::Ipv4Addr;

use anyhow::{bail, Context, Result};

/// 将点分十进制 IPv4 字符串解析为 `u32`。
pub fn ip_to_u32(ip: &str) -> Result<u32> {
    let addr: Ipv4Addr = ip
        .parse()
        .with_context(|| format!("无效的 IPv4 地址: {ip}"))?;
    Ok(u32::from(addr))
}

/// 将 `u32` 转换为点分十进制 IPv4 字符串。
pub fn u32_to_ip(val: u32) -> String {
    Ipv4Addr::from(val).to_string()
}

/// 解析 CIDR 字符串（如 `192.168.1.0/24`）为 `(网络地址, 前缀长度)`。
pub fn parse_cidr(cidr: &str) -> Result<(u32, u8)> {
    let (ip_str, prefix_str) = cidr
        .split_once('/')
        .with_context(|| format!("无效的 CIDR（缺少 /）: {cidr}"))?;
    let ip = ip_to_u32(ip_str)?;
    let prefix: u8 = prefix_str
        .parse()
        .with_context(|| format!("无效的前缀长度: {prefix_str}"))?;
    if prefix > 32 {
        bail!("前缀长度不能大于 32: {prefix}");
    }
    Ok((ip, prefix))
}

/// 根据前缀长度计算子网掩码。
pub fn netmask_from_prefix(prefix: u8) -> u32 {
    if prefix == 0 {
        0
    } else {
        (!0u32) << (32 - prefix)
    }
}

/// 根据子网掩码计算前缀长度，要求掩码连续。
pub fn prefix_from_netmask(mask: u32) -> Result<u8> {
    if mask == 0 {
        return Ok(0);
    }
    // 连续 1 掩码取反后形如 0...01...1，加 1 后为 2 的幂。
    let inv = !mask;
    if inv & (inv.wrapping_add(1)) != 0 {
        bail!("非连续的子网掩码: {}", u32_to_ip(mask));
    }
    Ok(mask.count_ones() as u8)
}

/// 计算给定 IP 与前缀对应的网络地址。
pub fn network_address(ip: u32, prefix: u8) -> u32 {
    ip & netmask_from_prefix(prefix)
}

/// 判断 IP 是否属于指定网段。
pub fn ip_in_network(ip: u32, network: u32, prefix: u8) -> bool {
    let mask = netmask_from_prefix(prefix);
    (ip & mask) == (network & mask)
}

/// 根据 IP 与子网掩码（点分十进制）推测 CIDR 网段。
///
/// 例如 `("192.168.1.5", "255.255.255.0")` -> `192.168.1.0/24`。
pub fn cidr_from_ip_mask(ip: &str, mask: &str) -> Result<String> {
    let ip_u = ip_to_u32(ip)?;
    let mask_u = ip_to_u32(mask)?;
    let prefix = prefix_from_netmask(mask_u)?;
    let net = ip_u & mask_u;
    Ok(format!("{}/{}", u32_to_ip(net), prefix))
}

/// 将真实 IP 换算为映射 IP。
///
/// 给定真实网段 `real_cidr` 与服务端分配的映射网段 `mapped_cidr`
/// （两者前缀长度必须一致），保留主机位，把真实 IP 映射到映射网段中。
///
/// 例如 `real_cidr=192.168.1.0/24`、`mapped_cidr=100.64.1.0/24`，
/// 真实 IP `192.168.1.5` -> 映射 IP `100.64.1.5`。
pub fn convert_real_to_mapped(real_ip: &str, real_cidr: &str, mapped_cidr: &str) -> Result<String> {
    let (_real_net, real_prefix) = parse_cidr(real_cidr)?;
    let (mapped_net, mapped_prefix) = parse_cidr(mapped_cidr)?;
    if real_prefix != mapped_prefix {
        bail!(
            "前缀长度不匹配: 真实 {real_prefix} vs 映射 {mapped_prefix}"
        );
    }
    let ip = ip_to_u32(real_ip)?;
    let mask = netmask_from_prefix(real_prefix);
    let host = ip & !mask;
    Ok(u32_to_ip(mapped_net | host))
}

/// 将映射 IP 换算为真实 IP（[`convert_real_to_mapped`] 的逆运算）。
pub fn convert_mapped_to_real(
    mapped_ip: &str,
    real_cidr: &str,
    mapped_cidr: &str,
) -> Result<String> {
    let (real_net, real_prefix) = parse_cidr(real_cidr)?;
    let (_mapped_net, mapped_prefix) = parse_cidr(mapped_cidr)?;
    if real_prefix != mapped_prefix {
        bail!(
            "前缀长度不匹配: 真实 {real_prefix} vs 映射 {mapped_prefix}"
        );
    }
    let ip = ip_to_u32(mapped_ip)?;
    let mask = netmask_from_prefix(real_prefix);
    let host = ip & !mask;
    Ok(u32_to_ip(real_net | host))
}

/// 判断一个 IP 字符串是否在某个 CIDR 网段内。
pub fn ip_in_cidr(ip: &str, cidr: &str) -> Result<bool> {
    let (net, prefix) = parse_cidr(cidr)?;
    let ip_u = ip_to_u32(ip)?;
    Ok(ip_in_network(ip_u, net, prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_mask() {
        assert_eq!(netmask_from_prefix(24), 0xFFFF_FF00);
        assert_eq!(prefix_from_netmask(0xFFFF_FF00).unwrap(), 24);
        assert_eq!(network_address(0xC0A8_0105, 24), 0xC0A8_0100);
    }

    #[test]
    fn cidr_from_mask() {
        assert_eq!(
            cidr_from_ip_mask("192.168.1.5", "255.255.255.0").unwrap(),
            "192.168.1.0/24"
        );
    }

    #[test]
    fn convert_roundtrip() {
        let real = "192.168.1.0/24";
        let mapped = "100.64.1.0/24";
        let m = convert_real_to_mapped("192.168.1.5", real, mapped).unwrap();
        assert_eq!(m, "100.64.1.5");
        let r = convert_mapped_to_real("100.64.1.5", real, mapped).unwrap();
        assert_eq!(r, "192.168.1.5");
    }
}
