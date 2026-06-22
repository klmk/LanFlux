//! net-tool-network
//!
//! 组网工具的网络转发核心模块。
//!
//! 基于 TUN 虚拟网卡 + TCP 隧道的三层转发系统，提供：
//! - [`ip`]：IPv4 / CIDR 换算工具
//! - [`tunnel`]：隧道帧协议，节点与服务端之间的通信协议
//! - [`tun`]：TUN 虚拟网卡管理
//! - [`route`]：跨平台路由添加与删除
//! - [`nat`]：NAT 配置（iptables / netsh）
//! - [`packet`]：IPv4 数据包解析与地址重写

/// IPv4 / CIDR 相关换算工具。
pub mod ip;
/// TUN 虚拟网卡管理。
pub mod tun;
/// 隧道帧协议。
pub mod tunnel;
/// 跨平台路由管理。
pub mod route;
/// NAT 配置。
pub mod nat;
/// IPv4 数据包解析。
pub mod packet;

pub use ip::*;
