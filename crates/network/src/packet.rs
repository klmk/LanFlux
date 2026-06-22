//! IPv4 数据包解析与地址重写
//!
//! 提供简单的 IPv4 包解析能力，用于识别包的源地址和目标地址，
//! 以及在转发时重写源/目标地址并重新计算校验和。
//!
//! ## IPv4 包头结构
//!
//! ```text
//!  0                   1                   2                   3
//!  0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
//! +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//! |Version|  IHL  |    DSCP/ECN   |         Total Length          |
//! +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//! |         Identification        |Flags|    Fragment Offset      |
//! +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//! |  TTL  |  Protocol  |       Header Checksum                    |
//! +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//! |                     Source IP Address                         |
//! +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//! |                  Destination IP Address                       |
//! +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//! ```

use std::net::Ipv4Addr;

use anyhow::{bail, Context, Result};

/// IP 协议号常量。
const PROTO_ICMP: u8 = 1;
const PROTO_TCP: u8 = 6;
const PROTO_UDP: u8 = 17;

/// 最小 IPv4 头部长度（无选项）。
const MIN_HEADER_LEN: usize = 20;

/// 解析后的 IPv4 数据包。
///
/// 仅提取转发所需的关键字段，保留原始数据。
#[derive(Debug, Clone)]
pub struct IpPacket {
    /// IP 版本（始终为 4）。
    pub version: u8,
    /// 上层协议号（1=ICMP, 6=TCP, 17=UDP）。
    pub protocol: u8,
    /// 源 IP 地址（点分十进制）。
    pub src_addr: String,
    /// 目标 IP 地址（点分十进制）。
    pub dst_addr: String,
    /// 原始数据（完整 IP 包）。
    pub raw: Vec<u8>,
}

impl IpPacket {
    /// 从字节流解析 IPv4 包。
    ///
    /// 返回 `None` 表示数据不是有效的 IPv4 包。
    /// 至少需要 20 字节（最小 IPv4 头部长度）。
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < MIN_HEADER_LEN {
            return None;
        }

        let version = data[0] >> 4;
        if version != 4 {
            return None;
        }

        let ihl = (data[0] & 0x0F) as usize;
        let header_len = ihl * 4;
        if header_len < MIN_HEADER_LEN || data.len() < header_len {
            return None;
        }

        let protocol = data[9];

        let src_addr = Ipv4Addr::new(data[12], data[13], data[14], data[15]).to_string();
        let dst_addr = Ipv4Addr::new(data[16], data[17], data[18], data[19]).to_string();

        Some(Self {
            version,
            protocol,
            src_addr,
            dst_addr,
            raw: data.to_vec(),
        })
    }

    /// 获取协议名称。
    pub fn protocol_name(&self) -> &'static str {
        match self.protocol {
            PROTO_ICMP => "ICMP",
            PROTO_TCP => "TCP",
            PROTO_UDP => "UDP",
            _ => "Unknown",
        }
    }
}

/// 重写 IP 包的目标地址。
///
/// 修改 `packet` 中偏移 16-19 字节的目标 IP，并重新计算 IP 头部校验和。
/// `packet` 必须是一个完整的 IPv4 包（至少 20 字节）。
pub fn rewrite_destination(packet: &mut [u8], new_dest: &str) -> Result<()> {
    if packet.len() < MIN_HEADER_LEN {
        bail!("IP 包长度不足: {} 字节（最少需要 {MIN_HEADER_LEN} 字节）", packet.len());
    }

    let dest: Ipv4Addr = new_dest
        .parse()
        .with_context(|| format!("无效的目标 IP 地址: {new_dest}"))?;

    packet[16] = dest.octets()[0];
    packet[17] = dest.octets()[1];
    packet[18] = dest.octets()[2];
    packet[19] = dest.octets()[3];

    recalculate_checksum(packet);

    tracing::trace!(new_dest = new_dest, "已重写 IP 包目标地址");
    Ok(())
}

/// 重写 IP 包的源地址。
///
/// 修改 `packet` 中偏移 12-15 字节的源 IP，并重新计算 IP 头部校验和。
/// `packet` 必须是一个完整的 IPv4 包（至少 20 字节）。
pub fn rewrite_source(packet: &mut [u8], new_source: &str) -> Result<()> {
    if packet.len() < MIN_HEADER_LEN {
        bail!("IP 包长度不足: {} 字节（最少需要 {MIN_HEADER_LEN} 字节）", packet.len());
    }

    let src: Ipv4Addr = new_source
        .parse()
        .with_context(|| format!("无效的源 IP 地址: {new_source}"))?;

    packet[12] = src.octets()[0];
    packet[13] = src.octets()[1];
    packet[14] = src.octets()[2];
    packet[15] = src.octets()[3];

    recalculate_checksum(packet);

    tracing::trace!(new_source = new_source, "已重写 IP 包源地址");
    Ok(())
}

/// 重新计算 IP 头部校验和。
///
/// 校验和范围为整个 IP 头部（IHL * 4 字节）。
/// 计算步骤：
/// 1. 将校验和字段（偏移 10-11）置零
/// 2. 对头部所有 16 位字求和
/// 3. 将进位折叠回低 16 位
/// 4. 取反得到最终校验和
fn recalculate_checksum(packet: &mut [u8]) {
    if packet.len() < MIN_HEADER_LEN {
        return;
    }

    let ihl = (packet[0] & 0x0F) as usize;
    let header_len = ihl * 4;
    if packet.len() < header_len {
        return;
    }

    // 校验和字段置零
    packet[10] = 0;
    packet[11] = 0;

    // 对头部所有 16 位字求和
    let mut sum: u32 = 0;
    let mut i = 0;
    while i + 1 < header_len {
        let word = ((packet[i] as u32) << 8) | (packet[i + 1] as u32);
        sum = sum.wrapping_add(word);
        i += 2;
    }

    // 如果头部长度为奇数，处理最后一个字节
    if header_len % 2 != 0 && header_len > 0 {
        sum = sum.wrapping_add((packet[header_len - 1] as u32) << 8);
    }

    // 折叠进位
    while (sum >> 16) != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }

    // 取反
    let checksum = !(sum as u16);
    packet[10] = (checksum >> 8) as u8;
    packet[11] = (checksum & 0xFF) as u8;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一个简单的 IPv4/ICMP 包用于测试。
    fn make_test_packet() -> Vec<u8> {
        let mut pkt = vec![
            0x45, // version=4, IHL=5
            0x00, // DSCP/ECN
            0x00, 0x1c, // total length = 28
            0x00, 0x01, // identification
            0x00, 0x00, // flags + fragment offset
            0x40, // TTL = 64
            0x01, // protocol = ICMP
            0x00, 0x00, // checksum (待计算)
            192, 168, 1, 1, // source IP
            10, 0, 0, 1, // destination IP
            // ICMP payload (8 bytes)
            0x08, 0x00, 0x00, 0x00, // type=8 (echo), code=0
            0x00, 0x01, 0x00, 0x00, // rest of header
        ];
        recalculate_checksum(&mut pkt);
        pkt
    }

    #[test]
    fn parse_valid_packet() {
        let pkt = make_test_packet();
        let parsed = IpPacket::parse(&pkt).unwrap();
        assert_eq!(parsed.version, 4);
        assert_eq!(parsed.protocol, 1);
        assert_eq!(parsed.protocol_name(), "ICMP");
        assert_eq!(parsed.src_addr, "192.168.1.1");
        assert_eq!(parsed.dst_addr, "10.0.0.1");
        assert_eq!(parsed.raw.len(), 28);
    }

    #[test]
    fn parse_too_short() {
        assert!(IpPacket::parse(&[0x45, 0x00]).is_none());
    }

    #[test]
    fn parse_wrong_version() {
        let mut pkt = make_test_packet();
        pkt[0] = 0x60; // version 6
        assert!(IpPacket::parse(&pkt).is_none());
    }

    #[test]
    fn protocol_names() {
        let mut pkt = make_test_packet();

        pkt[9] = 1;
        assert_eq!(IpPacket::parse(&pkt).unwrap().protocol_name(), "ICMP");

        pkt[9] = 6;
        assert_eq!(IpPacket::parse(&pkt).unwrap().protocol_name(), "TCP");

        pkt[9] = 17;
        assert_eq!(IpPacket::parse(&pkt).unwrap().protocol_name(), "UDP");

        pkt[9] = 99;
        assert_eq!(IpPacket::parse(&pkt).unwrap().protocol_name(), "Unknown");
    }

    #[test]
    fn rewrite_destination() {
        let mut pkt = make_test_packet();
        super::rewrite_destination(&mut pkt, "172.16.0.1").unwrap();

        let parsed = IpPacket::parse(&pkt).unwrap();
        assert_eq!(parsed.dst_addr, "172.16.0.1");
        assert_eq!(parsed.src_addr, "192.168.1.1"); // 源地址不变
    }

    #[test]
    fn rewrite_source() {
        let mut pkt = make_test_packet();
        super::rewrite_source(&mut pkt, "100.64.0.1").unwrap();

        let parsed = IpPacket::parse(&pkt).unwrap();
        assert_eq!(parsed.src_addr, "100.64.0.1");
        assert_eq!(parsed.dst_addr, "10.0.0.1"); // 目标地址不变
    }

    #[test]
    fn rewrite_both() {
        let mut pkt = make_test_packet();
        super::rewrite_source(&mut pkt, "100.64.0.1").unwrap();
        super::rewrite_destination(&mut pkt, "192.168.100.1").unwrap();

        let parsed = IpPacket::parse(&pkt).unwrap();
        assert_eq!(parsed.src_addr, "100.64.0.1");
        assert_eq!(parsed.dst_addr, "192.168.100.1");
    }

    #[test]
    fn checksum_valid_after_rewrite() {
        let mut pkt = make_test_packet();
        let original_checksum = u16::from_be_bytes([pkt[10], pkt[11]]);

        // 重写后校验和应该改变
        super::rewrite_destination(&mut pkt, "172.16.0.1").unwrap();
        let new_checksum = u16::from_be_bytes([pkt[10], pkt[11]]);
        assert_ne!(original_checksum, new_checksum);

        // 验证新校验和正确：对整个头部求和应为 0xFFFF
        let ihl = (pkt[0] & 0x0F) as usize;
        let header_len = ihl * 4;
        let mut sum: u32 = 0;
        for i in (0..header_len).step_by(2) {
            let word = ((pkt[i] as u32) << 8) | (pkt[i + 1] as u32);
            sum = sum.wrapping_add(word);
        }
        while (sum >> 16) != 0 {
            sum = (sum & 0xFFFF) + (sum >> 16);
        }
        assert_eq!(sum as u16, 0xFFFF);
    }

    #[test]
    fn rewrite_invalid_ip() {
        let mut pkt = make_test_packet();
        assert!(super::rewrite_destination(&mut pkt, "not-an-ip").is_err());
        assert!(super::rewrite_source(&mut pkt, "999.999.999.999").is_err());
    }

    #[test]
    fn rewrite_too_short() {
        let mut pkt = vec![0x45, 0x00];
        assert!(super::rewrite_destination(&mut pkt, "10.0.0.1").is_err());
        assert!(super::rewrite_source(&mut pkt, "10.0.0.1").is_err());
    }

    #[test]
    fn parse_with_options() {
        // IHL=6 (24 字节头部，含 4 字节选项)
        let mut pkt = vec![
            0x46, // version=4, IHL=6
            0x00,
            0x00, 0x18, // total length = 24
            0x00, 0x00,
            0x00, 0x00,
            0x40,
            0x06, // TCP
            0x00, 0x00,
            10, 0, 0, 1,
            10, 0, 0, 2,
            // 4 bytes options
            0x01, 0x01, 0x01, 0x00,
        ];
        recalculate_checksum(&mut pkt);
        let parsed = IpPacket::parse(&pkt).unwrap();
        assert_eq!(parsed.version, 4);
        assert_eq!(parsed.protocol_name(), "TCP");
        assert_eq!(parsed.src_addr, "10.0.0.1");
        assert_eq!(parsed.dst_addr, "10.0.0.2");
    }
}
