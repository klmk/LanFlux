use serde::{Deserialize, Serialize};

use crate::models::{
    AccessMode, NodeRole, OsType, RouteEntry, SegmentStatus,
};

// ============================================================
// 统一响应包装
// ============================================================

/// 统一 API 响应结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub code: i32,
    pub message: String,
    pub data: Option<T>,
}

impl<T> ApiResponse<T> {
    /// 成功响应
    pub fn ok(data: T) -> Self {
        ApiResponse {
            code: 0,
            message: "ok".to_string(),
            data: Some(data),
        }
    }

    /// 错误响应
    pub fn err(msg: impl Into<String>) -> Self {
        ApiResponse {
            code: -1,
            message: msg.into(),
            data: None,
        }
    }

    /// 带自定义错误码的错误响应
    pub fn err_with_code(code: i32, msg: impl Into<String>) -> Self {
        ApiResponse {
            code,
            message: msg.into(),
            data: None,
        }
    }
}

// ============================================================
// 节点相关请求/响应
// ============================================================

/// 节点注册请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub name: String,
    pub role: NodeRole,
    pub os_type: OsType,
    #[serde(default)]
    pub remark: Option<String>,
}

/// 节点注册响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterResponse {
    pub node_id: String,
    /// 实施端注册时分配的虚拟 IP，其他角色为 None
    pub virtual_ip: Option<String>,
}

/// 心跳请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatRequest {
    pub node_id: String,
    /// 当前节点上报的网段数量
    #[serde(default)]
    pub reported_segments_count: Option<i64>,
}

/// 心跳响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatResponse {
    /// 当前应下发给该节点的路由列表
    pub routes: Vec<RouteEntry>,
}

/// 修改节点请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateNodeRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub remark: Option<String>,
}

// ============================================================
// 网段相关请求/响应
// ============================================================

/// 上报网段请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSegmentRequest {
    pub node_id: String,
    pub name: String,
    /// 真实网段 CIDR，例如 "192.168.1.0/24"
    pub real_cidr: String,
    #[serde(default)]
    pub remark: Option<String>,
}

/// 上报网段响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSegmentResponse {
    pub segment_id: String,
    /// 服务端分配的映射网段 CIDR
    pub mapped_cidr: String,
}

/// 修改网段请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSegmentRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub remark: Option<String>,
}

// ============================================================
// 权限相关请求/响应
// ============================================================

/// 修改权限请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatePolicyRequest {
    pub access_mode: AccessMode,
    /// 允许访问的网段 ID 列表（当 access_mode 为 allowed_segments 时有效）
    #[serde(default)]
    pub allowed_segments: Vec<String>,
}

// ============================================================
// 地址池相关请求/响应
// ============================================================

/// 修改地址池请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatePoolRequest {
    #[serde(default)]
    pub client_pool_cidr: Option<String>,
    #[serde(default)]
    pub segment_size: Option<i32>,
    #[serde(default)]
    pub operator_pool_cidr: Option<String>,
    #[serde(default)]
    pub server_virtual_ip: Option<String>,
}

// ============================================================
// 网段查询 / 实施端地址申请 / 权限查询 请求/响应
// ============================================================

/// 查询节点已上报网段请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuerySegmentsRequest {
    pub node_id: String,
}

/// 网段摘要（查询列表用）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentSummary {
    /// 网段 ID
    pub id: String,
    /// 网段名称
    pub name: String,
    /// 真实网段 CIDR
    pub real_cidr: String,
    /// 服务端分配的映射网段 CIDR（未分配时为 None）
    #[serde(default)]
    pub mapped_cidr: Option<String>,
    /// 网段状态
    pub status: SegmentStatus,
    /// 备注
    #[serde(default)]
    pub remark: Option<String>,
}

/// 查询节点已上报网段响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuerySegmentsResponse {
    pub segments: Vec<SegmentSummary>,
}

/// 实施端申请虚拟 IP 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestOperatorIpRequest {
    pub node_id: String,
}

/// 实施端申请虚拟 IP 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestOperatorIpResponse {
    /// 分配给实施端的虚拟 IP
    pub virtual_ip: String,
    /// 当前可访问的网段路由列表
    pub accessible_segments: Vec<RouteEntry>,
}

/// 查询访问权限请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryAccessRequest {
    pub node_id: String,
}

/// 查询访问权限响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryAccessResponse {
    /// 访问模式
    pub access_mode: AccessMode,
    /// 允许访问的网段路由列表
    pub allowed_segments: Vec<RouteEntry>,
}
