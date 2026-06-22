//! 隧道帧协议
//!
//! 定义节点与服务端之间的通信协议。所有数据通过 TCP 连接传输，
//! 使用长度前缀帧格式进行分帧。
//!
//! ## 帧格式
//!
//! ```text
//! [4 bytes: payload_len (big-endian u32)][1 byte: frame_type][payload bytes]
//! ```
//!
//! `payload_len` 包含 1 字节的 `frame_type` 和 `payload` 的总长度。
//!
//! ## 帧类型
//!
//! | 类型 | 值 | 方向 | 说明 |
//! |------|-----|------|------|
//! | Auth | 0x01 | 客户端 -> 服务端 | 发送 node_id 认证 |
//! | AuthAck | 0x02 | 服务端 -> 客户端 | 回复认证结果和路由表 |
//! | IpPacket | 0x03 | 双向 | IPv4 数据包 |
//! | Heartbeat | 0x04 | 双向 | 心跳 |
//! | RouteUpdate | 0x05 | 服务端 -> 客户端 | 路由更新 |
//! | Disconnect | 0x06 | 双向 | 断开连接 |

use std::convert::TryFrom;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// 单个帧的最大 payload 长度（含 1 字节类型）。
/// IPv4 最大包长 65535，加上 1 字节类型，共 65536。
const MAX_PAYLOAD_LEN: usize = 65536;

/// 帧类型枚举。
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameType {
    /// 客户端发送 node_id 认证。
    Auth = 0x01,
    /// 服务端回复认证结果和路由表。
    AuthAck = 0x02,
    /// IPv4 数据包。
    IpPacket = 0x03,
    /// 心跳。
    Heartbeat = 0x04,
    /// 路由更新。
    RouteUpdate = 0x05,
    /// 断开连接。
    Disconnect = 0x06,
}

impl FrameType {
    /// 获取帧类型的字符串名称（用于日志）。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Auth => "Auth",
            Self::AuthAck => "AuthAck",
            Self::IpPacket => "IpPacket",
            Self::Heartbeat => "Heartbeat",
            Self::RouteUpdate => "RouteUpdate",
            Self::Disconnect => "Disconnect",
        }
    }
}

impl TryFrom<u8> for FrameType {
    type Error = String;

    fn try_from(value: u8) -> std::result::Result<Self, Self::Error> {
        match value {
            0x01 => Ok(Self::Auth),
            0x02 => Ok(Self::AuthAck),
            0x03 => Ok(Self::IpPacket),
            0x04 => Ok(Self::Heartbeat),
            0x05 => Ok(Self::RouteUpdate),
            0x06 => Ok(Self::Disconnect),
            _ => Err(format!("未知的帧类型: 0x{value:02X}")),
        }
    }
}

/// 隧道帧。
///
/// 由帧类型和 payload 组成。编码后通过 TCP 连接传输。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// 帧类型。
    pub frame_type: FrameType,
    /// 帧载荷数据。
    pub payload: Vec<u8>,
}

impl Frame {
    /// 创建一个新的帧。
    pub fn new(frame_type: FrameType, payload: Vec<u8>) -> Self {
        Self { frame_type, payload }
    }

    /// 编码帧为字节流。
    ///
    /// 格式：`[4 bytes: payload_len (big-endian u32)][1 byte: frame_type][payload bytes]`
    ///
    /// `payload_len` = 1（类型字节） + `payload.len()`。
    pub fn encode(&self) -> Vec<u8> {
        let payload_len = 1u32.checked_add(self.payload.len() as u32).unwrap_or(0);
        let mut buf = Vec::with_capacity(4 + 1 + self.payload.len());
        buf.extend_from_slice(&payload_len.to_be_bytes());
        buf.push(self.frame_type as u8);
        buf.extend_from_slice(&self.payload);
        buf
    }

    /// 从字节流解码一个完整的帧。
    ///
    /// `buf` 必须包含一个完整的帧（长度前缀 + 类型 + payload）。
    /// 返回 `None` 表示数据不完整或格式无效。
    pub fn decode(buf: &[u8]) -> Option<Self> {
        if buf.len() < 5 {
            return None;
        }
        let payload_len = u32::from_be_bytes([
            buf[0],
            buf[1],
            buf[2],
            buf[3],
        ]) as usize;
        if payload_len == 0 {
            return None;
        }
        // buf 需要包含 4 字节长度 + payload_len 字节数据
        if buf.len() < 4 + payload_len {
            return None;
        }
        let frame_type = FrameType::try_from(buf[4]).ok()?;
        let payload = buf[5..4 + payload_len].to_vec();
        Some(Self { frame_type, payload })
    }

    /// 创建认证帧。
    ///
    /// payload 为 `node_id` 的 UTF-8 字节。
    pub fn auth(node_id: &str) -> Self {
        Self::new(FrameType::Auth, node_id.as_bytes().to_vec())
    }

    /// 创建认证回复帧。
    ///
    /// payload: 1 字节 `success`（0 或 1） + `routes_json` 的 UTF-8 字节。
    pub fn auth_ack(success: bool, routes_json: &str) -> Self {
        let mut payload = Vec::with_capacity(1 + routes_json.len());
        payload.push(if success { 1 } else { 0 });
        payload.extend_from_slice(routes_json.as_bytes());
        Self::new(FrameType::AuthAck, payload)
    }

    /// 创建 IP 包帧。
    pub fn ip_packet(packet: &[u8]) -> Self {
        Self::new(FrameType::IpPacket, packet.to_vec())
    }

    /// 创建心跳帧。
    pub fn heartbeat() -> Self {
        Self::new(FrameType::Heartbeat, Vec::new())
    }

    /// 创建路由更新帧。
    ///
    /// payload 为 `routes_json` 的 UTF-8 字节。
    pub fn route_update(routes_json: &str) -> Self {
        Self::new(FrameType::RouteUpdate, routes_json.as_bytes().to_vec())
    }

    /// 创建断开连接帧。
    ///
    /// payload 为 `reason` 的 UTF-8 字节。
    pub fn disconnect(reason: &str) -> Self {
        Self::new(FrameType::Disconnect, reason.as_bytes().to_vec())
    }
}

/// 隧道路由条目。
///
/// 描述一条从映射网段到真实网段的路由映射关系，
/// 由服务端在认证回复或路由更新时下发给客户端。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelRoute {
    /// 映射网段 CIDR，如 `"100.64.1.0/24"`。
    pub mapped_cidr: String,
    /// 目标节点 ID。
    pub target_node_id: String,
    /// 目标节点名称。
    pub target_node_name: String,
    /// 真实网段 CIDR，如 `"192.168.1.0/24"`。
    pub real_cidr: String,
    /// 网段名称。
    pub segment_name: String,
}

/// 认证回复数据。
///
/// 服务端在认证通过后返回的完整数据，包含虚拟 IP、可访问路由列表等。
/// 可序列化为 JSON 字符串后通过 [`Frame::auth_ack`] 帧发送。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthAckData {
    /// 认证是否成功。
    pub success: bool,
    /// 实施端的虚拟 IP（认证成功时分配）。
    pub virtual_ip: Option<String>,
    /// 可访问的路由列表。
    pub routes: Vec<TunnelRoute>,
    /// 认证失败时的错误信息。
    pub error_message: Option<String>,
}

/// 隧道连接读取器。
///
/// 从实现了 [`AsyncRead`] 的流中逐帧读取数据。
/// 内部维护一个复用缓冲区以减少分配。
pub struct TunnelReader<R: AsyncRead + Unpin> {
    reader: R,
    buf: Vec<u8>,
}

impl<R: AsyncRead + Unpin> TunnelReader<R> {
    /// 创建读取器。
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            buf: Vec::new(),
        }
    }

    /// 异步读取一个完整的帧。
    ///
    /// 先读取 4 字节长度前缀，再读取对应长度的数据
    /// （1 字节类型 + payload），解析为 [`Frame`]。
    pub async fn read_frame(&mut self) -> Result<Frame> {
        // 读取 4 字节长度前缀
        let mut len_buf = [0u8; 4];
        self.reader
            .read_exact(&mut len_buf)
            .await
            .context("读取帧长度前缀失败")?;
        let payload_len = u32::from_be_bytes(len_buf) as usize;

        if payload_len == 0 {
            bail!("帧长度为 0（无效）");
        }
        if payload_len > MAX_PAYLOAD_LEN {
            bail!("帧长度过大: {payload_len}（最大 {MAX_PAYLOAD_LEN}）");
        }

        // 读取 payload_len 字节数据（1 字节类型 + 实际 payload）
        self.buf.resize(payload_len, 0);
        self.reader
            .read_exact(&mut self.buf)
            .await
            .context("读取帧数据失败")?;

        // 解析帧类型
        let frame_type = FrameType::try_from(self.buf[0])
            .map_err(|e| anyhow::anyhow!("无效的帧类型: {e}"))?;
        let payload = self.buf[1..].to_vec();

        tracing::trace!(
            frame_type = frame_type.as_str(),
            payload_len = payload.len(),
            "读取到隧道帧"
        );

        Ok(Frame { frame_type, payload })
    }
}

/// 隧道连接写入器。
///
/// 向实现了 [`AsyncWrite`] 的流中逐帧写入数据。
pub struct TunnelWriter<W: AsyncWrite + Unpin> {
    writer: W,
}

impl<W: AsyncWrite + Unpin> TunnelWriter<W> {
    /// 创建写入器。
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    /// 异步写入一个帧。
    ///
    /// 将帧编码为字节流后写入底层流。
    pub async fn write_frame(&mut self, frame: &Frame) -> Result<()> {
        let encoded = frame.encode();
        self.writer
            .write_all(&encoded)
            .await
            .with_context(|| format!("写入帧数据失败 (类型: {})", frame.frame_type.as_str()))?;

        tracing::trace!(
            frame_type = frame.frame_type.as_str(),
            total_len = encoded.len(),
            "已写入隧道帧"
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_type_roundtrip() {
        for v in 0x01..=0x06u8 {
            let ft = FrameType::try_from(v).unwrap();
            assert_eq!(ft as u8, v);
        }
        assert!(FrameType::try_from(0x00).is_err());
        assert!(FrameType::try_from(0x07).is_err());
    }

    #[test]
    fn encode_decode_basic() {
        let frame = Frame::new(FrameType::IpPacket, vec![1, 2, 3, 4, 5]);
        let encoded = frame.encode();
        // 4 (len) + 1 (type) + 5 (payload) = 10
        assert_eq!(encoded.len(), 10);
        assert_eq!(&encoded[0..4], &[0, 0, 0, 6]); // payload_len = 6
        assert_eq!(encoded[4], 0x03); // IpPacket

        let decoded = Frame::decode(&encoded).unwrap();
        assert_eq!(decoded, frame);
    }

    #[test]
    fn encode_decode_empty_payload() {
        let frame = Frame::heartbeat();
        let encoded = frame.encode();
        assert_eq!(encoded.len(), 5); // 4 (len) + 1 (type)
        assert_eq!(&encoded[0..4], &[0, 0, 0, 1]); // payload_len = 1

        let decoded = Frame::decode(&encoded).unwrap();
        assert_eq!(decoded, frame);
    }

    #[test]
    fn decode_incomplete() {
        assert!(Frame::decode(&[]).is_none());
        assert!(Frame::decode(&[0, 0, 0]).is_none());
        assert!(Frame::decode(&[0, 0, 0, 5, 0x03]).is_none()); // 声明 5 字节但只给了 1
    }

    #[test]
    fn decode_invalid_type() {
        // payload_len = 1, type = 0xFF (无效)
        let buf = [0, 0, 0, 1, 0xFF];
        assert!(Frame::decode(&buf).is_none());
    }

    #[test]
    fn auth_frame() {
        let frame = Frame::auth("node-123");
        assert_eq!(frame.frame_type, FrameType::Auth);
        assert_eq!(frame.payload, b"node-123");
    }

    #[test]
    fn auth_ack_frame() {
        let frame = Frame::auth_ack(true, r#"[{"mapped_cidr":"100.64.1.0/24"}]"#);
        assert_eq!(frame.frame_type, FrameType::AuthAck);
        assert_eq!(frame.payload[0], 1); // success = true
        assert_eq!(&frame.payload[1..], br#"[{"mapped_cidr":"100.64.1.0/24"}]"#);
    }

    #[test]
    fn ip_packet_frame() {
        let packet = [0x45, 0x00, 0x00, 0x14, 0, 0, 0, 0, 64, 6, 0, 0, 192, 168, 1, 1, 10, 0, 0, 1];
        let frame = Frame::ip_packet(&packet);
        assert_eq!(frame.frame_type, FrameType::IpPacket);
        assert_eq!(frame.payload, packet);
    }

    #[test]
    fn heartbeat_frame() {
        let frame = Frame::heartbeat();
        assert_eq!(frame.frame_type, FrameType::Heartbeat);
        assert!(frame.payload.is_empty());
    }

    #[test]
    fn route_update_frame() {
        let json = r#"{"routes":[]}"#;
        let frame = Frame::route_update(json);
        assert_eq!(frame.frame_type, FrameType::RouteUpdate);
        assert_eq!(frame.payload, json.as_bytes());
    }

    #[test]
    fn disconnect_frame() {
        let frame = Frame::disconnect("bye");
        assert_eq!(frame.frame_type, FrameType::Disconnect);
        assert_eq!(frame.payload, b"bye");
    }

    #[test]
    fn tunnel_route_serialize() {
        let route = TunnelRoute {
            mapped_cidr: "100.64.1.0/24".to_string(),
            target_node_id: "node-1".to_string(),
            target_node_name: "office".to_string(),
            real_cidr: "192.168.1.0/24".to_string(),
            segment_name: "办公网".to_string(),
        };
        let json = serde_json::to_string(&route).unwrap();
        let decoded: TunnelRoute = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.mapped_cidr, "100.64.1.0/24");
        assert_eq!(decoded.segment_name, "办公网");
    }

    #[test]
    fn auth_ack_data_serialize() {
        let data = AuthAckData {
            success: true,
            virtual_ip: Some("100.64.0.1".to_string()),
            routes: vec![],
            error_message: None,
        };
        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("100.64.0.1"));
    }

    #[tokio::test]
    async fn reader_writer_roundtrip() {
        use tokio::io::duplex;

        let (client, server) = duplex(1024);
        let mut writer = TunnelWriter::new(client);
        let mut reader = TunnelReader::new(server);

        let frame = Frame::ip_packet(&[0x45, 0x00, 0x00, 0x14]);
        writer.write_frame(&frame).await.unwrap();

        let decoded = reader.read_frame().await.unwrap();
        assert_eq!(decoded, frame);

        // 测试多帧连续读取
        let frame2 = Frame::auth("test-node");
        writer.write_frame(&frame2).await.unwrap();

        let decoded2 = reader.read_frame().await.unwrap();
        assert_eq!(decoded2, frame2);
    }

    #[tokio::test]
    async fn reader_writer_large_payload() {
        use tokio::io::duplex;

        let (client, server) = duplex(65536 * 2);
        let mut writer = TunnelWriter::new(client);
        let mut reader = TunnelReader::new(server);

        // 构造一个大 payload（接近最大值）
        let large_payload = vec![0xAB; 60000];
        let frame = Frame::ip_packet(&large_payload);
        writer.write_frame(&frame).await.unwrap();

        let decoded = reader.read_frame().await.unwrap();
        assert_eq!(decoded.payload, large_payload);
    }
}
