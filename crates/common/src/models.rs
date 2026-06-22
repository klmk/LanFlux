use serde::{Deserialize, Serialize};

// ============================================================
// 枚举类型
// ============================================================

/// 节点角色
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeRole {
    /// 服务端
    Server,
    /// 普通客户端
    Client,
    /// 实施端
    Operator,
}

impl NodeRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeRole::Server => "server",
            NodeRole::Client => "client",
            NodeRole::Operator => "operator",
        }
    }

    pub fn from_db_str(s: &str) -> Self {
        match s {
            "server" => NodeRole::Server,
            "operator" => NodeRole::Operator,
            _ => NodeRole::Client,
        }
    }
}

/// 节点连接状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeStatus {
    /// 已断开
    Disconnected,
    /// 已连接
    Connected,
    /// 已组网
    Networking,
    /// 部分异常
    PartialError,
}

impl NodeStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeStatus::Disconnected => "disconnected",
            NodeStatus::Connected => "connected",
            NodeStatus::Networking => "networking",
            NodeStatus::PartialError => "partial_error",
        }
    }

    pub fn from_db_str(s: &str) -> Self {
        match s {
            "connected" => NodeStatus::Connected,
            "networking" => NodeStatus::Networking,
            "partial_error" => NodeStatus::PartialError,
            _ => NodeStatus::Disconnected,
        }
    }
}

/// 操作系统类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OsType {
    Windows,
    Linux,
    Android,
    /// 未知 / 其它平台
    Unknown,
}

impl OsType {
    pub fn as_str(&self) -> &'static str {
        match self {
            OsType::Windows => "windows",
            OsType::Linux => "linux",
            OsType::Android => "android",
            OsType::Unknown => "unknown",
        }
    }

    pub fn from_db_str(s: &str) -> Self {
        match s {
            "windows" => OsType::Windows,
            "android" => OsType::Android,
            "unknown" => OsType::Unknown,
            _ => OsType::Linux,
        }
    }
}

/// 网段状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SegmentStatus {
    /// 待确认
    Pending,
    /// 已启用
    Active,
    /// 已停用
    Disabled,
    /// 异常
    Error,
}

impl SegmentStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            SegmentStatus::Pending => "pending",
            SegmentStatus::Active => "active",
            SegmentStatus::Disabled => "disabled",
            SegmentStatus::Error => "error",
        }
    }

    pub fn from_db_str(s: &str) -> Self {
        match s {
            "active" => SegmentStatus::Active,
            "disabled" => SegmentStatus::Disabled,
            "error" => SegmentStatus::Error,
            _ => SegmentStatus::Pending,
        }
    }
}

/// 访问权限模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccessMode {
    /// 禁止访问其他网段
    Denied,
    /// 允许访问指定网段
    AllowedSegments,
    /// 允许访问全部网段
    AllowedAll,
}

impl AccessMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            AccessMode::Denied => "denied",
            AccessMode::AllowedSegments => "allowed_segments",
            AccessMode::AllowedAll => "allowed_all",
        }
    }

    pub fn from_db_str(s: &str) -> Self {
        match s {
            "allowed_segments" => AccessMode::AllowedSegments,
            "allowed_all" => AccessMode::AllowedAll,
            _ => AccessMode::Denied,
        }
    }
}

/// 连接会话状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    /// 活跃
    Active,
    /// 超时
    Timeout,
    /// 已结束
    Ended,
}

impl SessionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            SessionStatus::Active => "active",
            SessionStatus::Timeout => "timeout",
            SessionStatus::Ended => "ended",
        }
    }

    pub fn from_db_str(s: &str) -> Self {
        match s {
            "timeout" => SessionStatus::Timeout,
            "ended" => SessionStatus::Ended,
            _ => SessionStatus::Active,
        }
    }
}

/// 网络协议
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Icmp,
    Tcp,
    Udp,
}

impl Protocol {
    pub fn as_str(&self) -> &'static str {
        match self {
            Protocol::Icmp => "icmp",
            Protocol::Tcp => "tcp",
            Protocol::Udp => "udp",
        }
    }

    pub fn from_db_str(s: &str) -> Self {
        match s {
            "tcp" => Protocol::Tcp,
            "udp" => Protocol::Udp,
            _ => Protocol::Icmp,
        }
    }
}

// ============================================================
// 数据模型
// ============================================================

/// 节点信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub id: String,
    pub name: String,
    pub role: NodeRole,
    pub status: NodeStatus,
    pub os_type: OsType,
    pub virtual_ip: Option<String>,
    pub reported_segments_count: i64,
    pub last_online: Option<String>,
    pub created_at: String,
    pub remark: String,
    pub enabled: bool,
}

/// 网段信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentInfo {
    pub id: String,
    pub node_id: String,
    pub name: String,
    pub real_cidr: String,
    pub mapped_cidr: String,
    pub status: SegmentStatus,
    pub remark: String,
    pub created_at: String,
    pub updated_at: String,
}

/// 权限策略信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessPolicyInfo {
    pub id: String,
    pub node_id: String,
    pub access_mode: AccessMode,
    pub allowed_segments: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// 地址池配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressPoolInfo {
    pub id: i64,
    pub client_pool_cidr: String,
    pub segment_size: i32,
    pub operator_pool_cidr: String,
    pub server_virtual_ip: String,
    pub next_client_segment_index: i64,
    pub next_operator_ip_index: i64,
}

/// 连接会话信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionSessionInfo {
    pub id: String,
    pub source_node_id: String,
    pub target_segment_id: String,
    pub protocol: Protocol,
    pub target_address: String,
    pub target_client_id: String,
    pub started_at: String,
    pub last_activity: String,
    pub status: SessionStatus,
}

/// 仪表盘数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardData {
    /// 在线节点数
    pub online_node_count: i64,
    /// 普通客户端数
    pub client_count: i64,
    /// 实施端数
    pub operator_count: i64,
    /// 已登记网段数
    pub registered_segment_count: i64,
    /// 已启用映射网段数
    pub active_mapped_segment_count: i64,
    /// 异常网段数
    pub error_segment_count: i64,
    /// 当前连接会话数
    pub active_session_count: i64,
}

/// 路由条目（下发给节点的路由）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteEntry {
    /// 映射网段 CIDR
    pub mapped_cidr: String,
    /// 目标节点 ID
    pub target_node_id: String,
    /// 目标节点名称
    pub target_node_name: String,
    /// 真实网段 CIDR
    pub real_cidr: String,
    /// 网段名称
    #[serde(default)]
    pub segment_name: String,
}

/// 访问权限查询响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessQueryResponse {
    pub node_id: String,
    pub access_mode: AccessMode,
    pub routes: Vec<RouteEntry>,
}

/// 映射关系信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MappingInfo {
    pub segment_id: String,
    pub node_id: String,
    pub node_name: String,
    pub segment_name: String,
    pub real_cidr: String,
    pub mapped_cidr: String,
    pub status: SegmentStatus,
    /// 访问示例，例如 "192.168.1.10 -> 100.64.1.10"
    pub access_example: String,
}

/// 日志条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub module: String,
    pub message: String,
}

/// 地址池配置（用于配置文件，不含数据库自增 id）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AddressPool {
    /// 客户端映射网段地址池 CIDR，如 `100.64.0.0/16`
    pub client_pool_cidr: String,
    /// 单个映射网段大小（前缀长度，如 24）
    pub segment_size: i32,
    /// 实施端虚拟 IP 地址池 CIDR，如 `10.244.0.0/16`
    pub operator_pool_cidr: String,
    /// 服务端自身虚拟 IP
    pub server_virtual_ip: String,
}
