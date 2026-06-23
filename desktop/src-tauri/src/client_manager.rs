//! 桌面端客户端管理器
//!
//! 桥接 Tauri 前端与 Rust 客户端核心功能：
//! - 管理服务端连接（注册、心跳）
//! - 管理隧道客户端（认证、TUN 创建、数据包转发）
//! - 提供状态查询、网段上报、连通性测试等接口

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use net_tool_client::cli::detect_os;
use net_tool_client::connection::{run_heartbeat, ConnectionStatus, ServerConnection};
use net_tool_client::tunnel_client::{get_tunnel_addr, TunnelClient, TunnelHandle, TunnelStatus};
use net_tool_common::{
    NodeRole, OsType, RegisterRequest, ReportSegmentResponse, RequestOperatorIpResponse,
    RouteEntry, SegmentSummary,
};

// ============================================================
// 前端交互数据结构
// ============================================================

/// 运行模式
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunMode {
    Idle,
    Client,
    Operator,
}

/// 综合状态信息（返回给前端）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusInfo {
    pub mode: String,
    pub connection_status: String,
    pub tunnel_status: String,
    pub node_id: Option<String>,
    pub node_name: Option<String>,
    pub virtual_ip: Option<String>,
    pub server_addr: Option<String>,
    pub accessible_segments_count: usize,
    pub reported_segments_count: usize,
}

/// 启动连接结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectResult {
    pub node_id: String,
    pub virtual_ip: Option<String>,
    pub accessible_segments: Vec<RouteEntry>,
}

/// 隧道启动结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelStartResult {
    pub virtual_ip: Option<String>,
    pub routes: Vec<RouteInfo>,
}

/// 路由信息（简化版，返回给前端）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteInfo {
    pub target_node_id: String,
    pub target_node_name: String,
    pub segment_name: String,
    pub real_cidr: String,
    pub mapped_cidr: String,
}

impl From<&net_tool_network::tunnel::TunnelRoute> for RouteInfo {
    fn from(r: &net_tool_network::tunnel::TunnelRoute) -> Self {
        Self {
            target_node_id: r.target_node_id.clone(),
            target_node_name: r.target_node_name.clone(),
            segment_name: r.segment_name.clone(),
            real_cidr: r.real_cidr.clone(),
            mapped_cidr: r.mapped_cidr.clone(),
        }
    }
}

/// Ping 测试结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingResult {
    pub success: bool,
    pub target: String,
    pub elapsed_ms: u64,
    pub message: String,
}

/// TCP 测试结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpTestResult {
    pub success: bool,
    pub target: String,
    pub port: u16,
    pub elapsed_ms: u64,
    pub message: String,
}

/// IP 转换结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertIpResult {
    pub input_ip: String,
    pub real_cidr: Option<String>,
    pub mapped_cidr: Option<String>,
    pub mapped_ip: Option<String>,
    pub message: String,
}

// ============================================================
// 客户端管理器
// ============================================================

/// 客户端管理器内部状态
struct ClientManagerInner {
    /// 服务端连接
    conn: Option<ServerConnection>,
    /// 隧道句柄（用于状态查询和停止）
    tunnel_handle: Option<TunnelHandle>,
    /// 隧道后台任务
    tunnel_task: Option<tokio::task::JoinHandle<()>>,
    /// 心跳运行标志
    heartbeat_running: Option<Arc<AtomicBool>>,
    /// 心跳后台任务
    heartbeat_task: Option<tokio::task::JoinHandle<()>>,
    /// 连接状态共享
    conn_status: Arc<Mutex<ConnectionStatus>>,
    /// 当前运行模式
    mode: RunMode,
    /// 节点 ID
    node_id: Option<String>,
    /// 节点名称
    node_name: Option<String>,
    /// 虚拟 IP
    virtual_ip: Option<String>,
    /// 服务端地址
    server_addr: Option<String>,
    /// 可访问网段（实施端）
    accessible_segments: Vec<RouteEntry>,
    /// 已上报网段（客户端）
    reported_segments: Vec<SegmentSummary>,
}

impl Default for ClientManagerInner {
    fn default() -> Self {
        Self {
            conn: None,
            tunnel_handle: None,
            tunnel_task: None,
            heartbeat_running: None,
            heartbeat_task: None,
            conn_status: Arc::new(Mutex::new(ConnectionStatus::Disconnected)),
            mode: RunMode::Idle,
            node_id: None,
            node_name: None,
            virtual_ip: None,
            server_addr: None,
            accessible_segments: Vec::new(),
            reported_segments: Vec::new(),
        }
    }
}

/// 客户端管理器
///
/// 线程安全，可在 Tauri 命令间共享。
#[derive(Clone)]
pub struct ClientManager {
    inner: Arc<Mutex<ClientManagerInner>>,
}

impl ClientManager {
    /// 创建空的客户端管理器
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(ClientManagerInner::default())),
        }
    }

    /// 以客户端模式连接服务端
    ///
    /// 1. 创建 ServerConnection
    /// 2. 注册节点（client 角色）
    /// 3. 启动心跳循环
    pub async fn connect_client(
        &self,
        server_addr: &str,
        node_name: &str,
        remark: Option<String>,
    ) -> Result<ConnectResult, String> {
        let mut inner = self.inner.lock().await;

        // 如果已有连接，先断开
        if inner.conn.is_some() {
            Self::stop_all(&mut inner).await;
        }

        // 创建连接
        let mut conn = ServerConnection::new(server_addr)
            .map_err(|e| format!("创建连接失败: {e}"))?;

        // 注册
        let reg_req = RegisterRequest {
            name: node_name.to_string(),
            role: NodeRole::Client,
            os_type: detect_os(),
            remark,
        };

        let reg_resp = conn
            .register(reg_req)
            .await
            .map_err(|e| format!("注册失败: {e}"))?;

        let node_id = reg_resp.node_id.clone();
        let virtual_ip = reg_resp.virtual_ip.clone();

        // 更新共享状态
        *inner.conn_status.lock().await = ConnectionStatus::Connected;

        // 启动心跳
        let heartbeat_running = Arc::new(AtomicBool::new(true));
        let heartbeat_conn = conn.clone();
        let heartbeat_status = inner.conn_status.clone();
        let hb_running = heartbeat_running.clone();
        let heartbeat_task = tokio::spawn(async move {
            run_heartbeat(heartbeat_conn, heartbeat_status, hb_running).await;
        });

        // 保存状态
        inner.conn = Some(conn);
        inner.heartbeat_running = Some(heartbeat_running);
        inner.heartbeat_task = Some(heartbeat_task);
        inner.mode = RunMode::Client;
        inner.node_id = Some(node_id.clone());
        inner.node_name = Some(node_name.to_string());
        inner.virtual_ip = virtual_ip.clone();
        inner.server_addr = Some(server_addr.to_string());

        // 查询已上报网段
        let reported = if let Some(ref conn) = inner.conn {
            conn.query_segments().await.unwrap_or_default()
        } else {
            Vec::new()
        };
        inner.reported_segments = reported.clone();

        Ok(ConnectResult {
            node_id,
            virtual_ip,
            accessible_segments: Vec::new(),
        })
    }

    /// 以实施端模式连接服务端
    ///
    /// 1. 创建 ServerConnection
    /// 2. 注册节点（operator 角色，自动分配虚拟 IP）
    /// 3. 申请可访问网段
    /// 4. 启动心跳循环
    pub async fn connect_operator(
        &self,
        server_addr: &str,
        node_name: &str,
    ) -> Result<ConnectResult, String> {
        let mut inner = self.inner.lock().await;

        // 如果已有连接，先断开
        if inner.conn.is_some() {
            Self::stop_all(&mut inner).await;
        }

        // 创建连接
        let mut conn = ServerConnection::new(server_addr)
            .map_err(|e| format!("创建连接失败: {e}"))?;

        // 注册
        let reg_req = RegisterRequest {
            name: node_name.to_string(),
            role: NodeRole::Operator,
            os_type: detect_os(),
            remark: None,
        };

        let reg_resp = conn
            .register(reg_req)
            .await
            .map_err(|e| format!("注册失败: {e}"))?;

        let node_id = reg_resp.node_id.clone();
        let virtual_ip = reg_resp.virtual_ip.clone();

        // 申请可访问网段
        let ip_resp: RequestOperatorIpResponse = conn
            .request_operator_ip()
            .await
            .map_err(|e| format!("申请实施端 IP 失败: {e}"))?;

        let accessible_segments = ip_resp.accessible_segments.clone();
        let virtual_ip = ip_resp.virtual_ip;

        // 更新共享状态
        *inner.conn_status.lock().await = ConnectionStatus::Connected;

        // 启动心跳
        let heartbeat_running = Arc::new(AtomicBool::new(true));
        let heartbeat_conn = conn.clone();
        let heartbeat_status = inner.conn_status.clone();
        let hb_running = heartbeat_running.clone();
        let heartbeat_task = tokio::spawn(async move {
            run_heartbeat(heartbeat_conn, heartbeat_status, hb_running).await;
        });

        // 保存状态
        inner.conn = Some(conn);
        inner.heartbeat_running = Some(heartbeat_running);
        inner.heartbeat_task = Some(heartbeat_task);
        inner.mode = RunMode::Operator;
        inner.node_id = Some(node_id.clone());
        inner.node_name = Some(node_name.to_string());
        inner.virtual_ip = Some(virtual_ip.clone());
        inner.server_addr = Some(server_addr.to_string());
        inner.accessible_segments = accessible_segments.clone();

        Ok(ConnectResult {
            node_id,
            virtual_ip: Some(virtual_ip),
            accessible_segments,
        })
    }

    /// 启动隧道
    ///
    /// 在已连接服务端的前提下，创建隧道客户端并启动转发。
    pub async fn start_tunnel(&self) -> Result<TunnelStartResult, String> {
        let mut inner = self.inner.lock().await;

        // 检查是否已连接
        let node_id = inner
            .node_id
            .clone()
            .ok_or_else(|| "未连接服务端，请先连接".to_string())?;
        let server_addr = inner
            .server_addr
            .clone()
            .ok_or_else(|| "未连接服务端，请先连接".to_string())?;
        let is_operator = matches!(inner.mode, RunMode::Operator);

        // 如果隧道已在运行，先停止
        if inner.tunnel_handle.is_some() {
            Self::stop_tunnel_inner(&mut inner).await;
        }

        // 创建隧道客户端
        let tunnel_addr = get_tunnel_addr(&server_addr);
        let mut tunnel = TunnelClient::new(node_id, tunnel_addr, is_operator);

        // 连接隧道
        let ack = tunnel
            .connect()
            .await
            .map_err(|e| format!("隧道连接失败: {e}"))?;

        let virtual_ip = ack.virtual_ip.clone();
        let routes: Vec<RouteInfo> = ack.routes.iter().map(RouteInfo::from).collect();

        // 提取句柄
        let handle = tunnel.handle();

        // 启动隧道主循环
        let tunnel_task = tokio::spawn(async move {
            if let Err(e) = tunnel.run().await {
                tracing::error!("隧道运行失败: {e}");
            }
        });

        // 保存状态
        inner.tunnel_handle = Some(handle);
        inner.tunnel_task = Some(tunnel_task);

        // 更新虚拟 IP
        if virtual_ip.is_some() {
            inner.virtual_ip = virtual_ip.clone();
        }

        Ok(TunnelStartResult { virtual_ip, routes })
    }

    /// 停止隧道
    pub async fn stop_tunnel(&self) -> Result<(), String> {
        let mut inner = self.inner.lock().await;
        Self::stop_tunnel_inner(&mut inner).await;
        Ok(())
    }

    /// 断开所有连接
    pub async fn disconnect(&self) -> Result<(), String> {
        let mut inner = self.inner.lock().await;
        Self::stop_all(&mut inner).await;
        Ok(())
    }

    /// 获取综合状态
    pub async fn get_status(&self) -> StatusInfo {
        // 先在锁内快速提取需要的数据，然后释放锁
        let (conn_status_arc, tunnel_handle, mode, node_id, node_name, virtual_ip, server_addr, accessible_count, reported_count) = {
            let inner = self.inner.lock().await;
            (
                inner.conn_status.clone(),
                inner.tunnel_handle.clone(),
                inner.mode.clone(),
                inner.node_id.clone(),
                inner.node_name.clone(),
                inner.virtual_ip.clone(),
                inner.server_addr.clone(),
                inner.accessible_segments.len(),
                inner.reported_segments.len(),
            )
        };

        // 在锁外执行异步操作
        let connection_status = conn_status_arc.lock().await.clone();
        let tunnel_status = if let Some(ref handle) = tunnel_handle {
            handle.get_status().await
        } else {
            TunnelStatus::Disconnected
        };

        StatusInfo {
            mode: match mode {
                RunMode::Idle => "idle".to_string(),
                RunMode::Client => "client".to_string(),
                RunMode::Operator => "operator".to_string(),
            },
            connection_status: connection_status.as_str().to_string(),
            tunnel_status: tunnel_status.as_str().to_string(),
            node_id,
            node_name,
            virtual_ip,
            server_addr,
            accessible_segments_count: accessible_count,
            reported_segments_count: reported_count,
        }
    }

    /// 获取隧道路由
    pub async fn get_tunnel_routes(&self) -> Vec<RouteInfo> {
        let handle = {
            let inner = self.inner.lock().await;
            inner.tunnel_handle.clone()
        };
        if let Some(handle) = handle {
            let routes = handle.get_routes().await;
            routes.iter().map(RouteInfo::from).collect()
        } else {
            Vec::new()
        }
    }

    /// 获取可访问网段（实施端）
    pub async fn get_accessible_segments(&self) -> Vec<RouteEntry> {
        let inner = self.inner.lock().await;
        inner.accessible_segments.clone()
    }

    /// 刷新可访问网段（从服务端重新查询）
    pub async fn refresh_accessible_segments(&self) -> Result<Vec<RouteEntry>, String> {
        let conn = {
            let inner = self.inner.lock().await;
            inner.conn.clone()
        };
        if let Some(conn) = conn {
            let resp = conn
                .query_access()
                .await
                .map_err(|e| format!("查询访问权限失败: {e}"))?;
            let segments = resp.allowed_segments.clone();
            let mut inner = self.inner.lock().await;
            inner.accessible_segments = segments.clone();
            Ok(segments)
        } else {
            Err("未连接服务端".to_string())
        }
    }

    /// 获取已上报网段（客户端）
    pub async fn get_reported_segments(&self) -> Vec<SegmentSummary> {
        let inner = self.inner.lock().await;
        inner.reported_segments.clone()
    }

    /// 刷新已上报网段（从服务端重新查询）
    pub async fn refresh_reported_segments(&self) -> Result<Vec<SegmentSummary>, String> {
        let conn = {
            let inner = self.inner.lock().await;
            inner.conn.clone()
        };
        if let Some(conn) = conn {
            let segments = conn
                .query_segments()
                .await
                .map_err(|e| format!("查询网段失败: {e}"))?;
            let result = segments.clone();
            let mut inner = self.inner.lock().await;
            inner.reported_segments = segments;
            Ok(result)
        } else {
            Err("未连接服务端".to_string())
        }
    }

    /// 上报网段
    pub async fn report_segment(
        &self,
        name: &str,
        real_cidr: &str,
        remark: Option<String>,
    ) -> Result<ReportSegmentResponse, String> {
        let conn = {
            let inner = self.inner.lock().await;
            inner.conn.clone()
        };
        if let Some(conn) = conn {
            let resp = conn
                .report_segment(name, real_cidr, remark)
                .await
                .map_err(|e| format!("上报网段失败: {e}"))?;
            Ok(resp)
        } else {
            Err("未连接服务端".to_string())
        }
    }

    /// Ping 测试（通过系统 ping 命令）
    pub async fn ping_test(&self, target_ip: &str) -> PingResult {
        let start = std::time::Instant::now();

        // 跨平台 ping 命令参数
        #[cfg(target_os = "windows")]
        let (count_flag, count_val, timeout_flag, timeout_val) = ("-n", "4", "-w", "3000");
        #[cfg(not(target_os = "windows"))]
        let (count_flag, count_val, timeout_flag, timeout_val) = ("-c", "4", "-W", "3");

        let output = tokio::process::Command::new("ping")
            .args([count_flag, count_val, timeout_flag, timeout_val, target_ip])
            .output()
            .await;

        match output {
            Ok(out) => {
                let elapsed = start.elapsed().as_millis() as u64;
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();

                if out.status.success() {
                    // 提取统计信息
                    let stats = extract_ping_stats(&stdout);
                    PingResult {
                        success: true,
                        target: target_ip.to_string(),
                        elapsed_ms: elapsed,
                        message: stats,
                    }
                } else {
                    PingResult {
                        success: false,
                        target: target_ip.to_string(),
                        elapsed_ms: elapsed,
                        message: if stdout.is_empty() {
                            stderr
                        } else {
                            stdout
                        },
                    }
                }
            }
            Err(e) => PingResult {
                success: false,
                target: target_ip.to_string(),
                elapsed_ms: start.elapsed().as_millis() as u64,
                message: format!("执行 ping 命令失败: {e}"),
            },
        }
    }

    /// TCP 连通性测试
    pub async fn tcp_test(&self, target_ip: &str, port: u16) -> TcpTestResult {
        let start = std::time::Instant::now();
        let addr = format!("{target_ip}:{port}");

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            tokio::net::TcpStream::connect(&addr),
        )
        .await;

        let elapsed = start.elapsed().as_millis() as u64;

        match result {
            Ok(Ok(_)) => TcpTestResult {
                success: true,
                target: target_ip.to_string(),
                port,
                elapsed_ms: elapsed,
                message: format!("成功连接 {addr}（{elapsed}ms）"),
            },
            Ok(Err(e)) => TcpTestResult {
                success: false,
                target: target_ip.to_string(),
                port,
                elapsed_ms: elapsed,
                message: format!("连接失败: {e}"),
            },
            Err(_) => TcpTestResult {
                success: false,
                target: target_ip.to_string(),
                port,
                elapsed_ms: elapsed,
                message: "连接超时（5秒）".to_string(),
            },
        }
    }

    /// IP 转换：根据可访问网段，将真实 IP 转换为映射 IP
    pub async fn convert_ip(&self, input_ip: &str) -> ConvertIpResult {
        let accessible = {
            let inner = self.inner.lock().await;
            inner.accessible_segments.clone()
        };

        // 在可访问网段中查找匹配的映射关系
        for seg in &accessible {
            // 检查输入 IP 是否在真实网段内
            if let Ok(true) = ip_in_cidr(input_ip, &seg.real_cidr) {
                // 计算映射 IP
                if let Some(mapped_ip) = convert_ip_in_segment(input_ip, &seg.real_cidr, &seg.mapped_cidr) {
                    return ConvertIpResult {
                        input_ip: input_ip.to_string(),
                        real_cidr: Some(seg.real_cidr.clone()),
                        mapped_cidr: Some(seg.mapped_cidr.clone()),
                        mapped_ip: Some(mapped_ip.clone()),
                        message: format!(
                            "{} ({}) -> {} ({})",
                            input_ip, seg.real_cidr, mapped_ip, seg.mapped_cidr
                        ),
                    };
                }
            }
        }

        ConvertIpResult {
            input_ip: input_ip.to_string(),
            real_cidr: None,
            mapped_cidr: None,
            mapped_ip: None,
            message: format!("IP {input_ip} 不在任何已知网段中，或当前无可访问网段"),
        }
    }

    /// 获取连接状态
    pub async fn get_connection_status(&self) -> ConnectionStatus {
        let conn_status = {
            let inner = self.inner.lock().await;
            inner.conn_status.clone()
        };
        conn_status.lock().await.clone()
    }

    /// 获取隧道状态
    pub async fn get_tunnel_status(&self) -> TunnelStatus {
        let handle = {
            let inner = self.inner.lock().await;
            inner.tunnel_handle.clone()
        };
        if let Some(handle) = handle {
            handle.get_status().await
        } else {
            TunnelStatus::Disconnected
        }
    }

    // ============================================================
    // 内部辅助方法
    // ============================================================

    /// 停止隧道（内部方法，不加锁）
    async fn stop_tunnel_inner(inner: &mut ClientManagerInner) {
        if let Some(handle) = inner.tunnel_handle.take() {
            handle.stop().await;
        }
        if let Some(task) = inner.tunnel_task.take() {
            task.abort();
        }
    }

    /// 停止所有连接和任务（内部方法，不加锁）
    async fn stop_all(inner: &mut ClientManagerInner) {
        // 停止隧道
        Self::stop_tunnel_inner(inner).await;

        // 停止心跳
        if let Some(running) = inner.heartbeat_running.take() {
            running.store(false, Ordering::SeqCst);
        }
        if let Some(task) = inner.heartbeat_task.take() {
            task.abort();
        }

        // 更新状态
        *inner.conn_status.lock().await = ConnectionStatus::Disconnected;

        // 清理
        inner.conn = None;
        inner.mode = RunMode::Idle;
        inner.node_id = None;
        inner.node_name = None;
        inner.virtual_ip = None;
        inner.server_addr = None;
        inner.accessible_segments.clear();
        inner.reported_segments.clear();
    }
}

impl Default for ClientManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// 辅助函数
// ============================================================

/// 从 ping 输出中提取统计信息
fn extract_ping_stats(stdout: &str) -> String {
    // 尝试提取最后一行统计信息
    for line in stdout.lines().rev() {
        if line.contains("packets transmitted") || line.contains("rtt") || line.contains("min/avg/max") {
            return line.to_string();
        }
    }
    // 如果没找到统计行，返回整个输出（截断）
    if stdout.len() > 200 {
        format!("{}...", &stdout[..200])
    } else {
        stdout.to_string()
    }
}

/// 检查 IP 是否在 CIDR 网段内
fn ip_in_cidr(ip: &str, cidr: &str) -> Result<bool, String> {
    let parts: Vec<&str> = cidr.split('/').collect();
    if parts.len() != 2 {
        return Err(format!("CIDR 格式错误: {cidr}"));
    }

    let network: u32 = ip_to_u32(parts[0]).ok_or_else(|| format!("IP 格式错误: {}", parts[0]))?;
    let prefix: u32 = parts[1]
        .parse()
        .map_err(|e| format!("前缀长度解析失败: {e}"))?;
    let mask: u32 = if prefix == 0 {
        0
    } else {
        !0u32 << (32 - prefix)
    };

    let target: u32 = ip_to_u32(ip).ok_or_else(|| format!("IP 格式错误: {ip}"))?;
    Ok((target & mask) == (network & mask))
}

/// 将 IP 地址转换为 u32
fn ip_to_u32(ip: &str) -> Option<u32> {
    let parts: Vec<&str> = ip.split('.').collect();
    if parts.len() != 4 {
        return None;
    }
    let mut result: u32 = 0;
    for part in parts {
        let octet: u32 = part.parse().ok()?;
        if octet > 255 {
            return None;
        }
        result = (result << 8) | octet;
    }
    Some(result)
}

/// 将 u32 转换为 IP 地址
fn u32_to_ip(val: u32) -> String {
    format!(
        "{}.{}.{}.{}",
        (val >> 24) & 0xff,
        (val >> 16) & 0xff,
        (val >> 8) & 0xff,
        val & 0xff
    )
}

/// 在网段映射关系中转换 IP
///
/// 例如：real_cidr = "192.168.1.0/24", mapped_cidr = "100.64.1.0/24"
/// 输入 "192.168.1.100" -> 输出 "100.64.1.100"
fn convert_ip_in_segment(ip: &str, real_cidr: &str, mapped_cidr: &str) -> Option<String> {
    let real_parts: Vec<&str> = real_cidr.split('/').collect();
    let mapped_parts: Vec<&str> = mapped_cidr.split('/').collect();
    if real_parts.len() != 2 || mapped_parts.len() != 2 {
        return None;
    }

    let real_network = ip_to_u32(real_parts[0])?;
    let mapped_network = ip_to_u32(mapped_parts[0])?;
    let prefix: u32 = real_parts[1].parse().ok()?;
    let mask: u32 = if prefix == 0 {
        0
    } else {
        !0u32 << (32 - prefix)
    };

    let target = ip_to_u32(ip)?;
    let host_part = target & !mask;
    let mapped_ip = mapped_network | host_part;

    Some(u32_to_ip(mapped_ip))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ip_in_cidr() {
        assert!(ip_in_cidr("192.168.1.100", "192.168.1.0/24").unwrap());
        assert!(!ip_in_cidr("192.168.2.100", "192.168.1.0/24").unwrap());
        assert!(ip_in_cidr("100.64.1.50", "100.64.1.0/24").unwrap());
    }

    #[test]
    fn test_convert_ip() {
        let result = convert_ip_in_segment("192.168.1.100", "192.168.1.0/24", "100.64.1.0/24");
        assert_eq!(result, Some("100.64.1.100".to_string()));

        let result = convert_ip_in_segment("192.168.1.1", "192.168.1.0/24", "100.64.1.0/24");
        assert_eq!(result, Some("100.64.1.1".to_string()));
    }
}
