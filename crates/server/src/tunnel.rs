//! 隧道中转服务器模块
//!
//! 服务端在 TCP 端口（默认 8444）上监听隧道连接，接收客户端和实施端的
//! 隧道连接，在它们之间转发 IP 数据包。
//!
//! ## 工作流程
//!
//! 1. 接受 TCP 连接，使用 `tokio::io::split` 分割为读/写两半
//! 2. 等待认证帧（10 秒超时），验证 node_id 是否存在且已启用
//! 3. 查询数据库获取节点角色、名称、虚拟 IP 和可访问路由
//! 4. 发送 AuthAck 帧（包含成功状态、虚拟 IP、路由列表）
//! 5. 将节点加入 `TunnelState`，广播路由更新
//! 6. 进入主循环：读取帧并处理（IP 包转发 / 心跳 / 断开）
//! 7. 心跳超时（90 秒无数据）或连接断开时清理节点

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use sqlx::{Row, SqlitePool};
use tokio::io::{ReadHalf, WriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, RwLock};

use net_tool_common::NodeStatus;
use net_tool_network::packet::IpPacket;
use net_tool_network::tunnel::{AuthAckData, Frame, FrameType, TunnelReader, TunnelRoute, TunnelWriter};

// ============================================================
// 类型别名
// ============================================================

/// 隧道读取器（持有 TcpStream 的读半部）
type TunnelReaderHalf = TunnelReader<ReadHalf<TcpStream>>;

/// 隧道写入器（持有 TcpStream 的写半部）
type TunnelWriterHalf = TunnelWriter<WriteHalf<TcpStream>>;

// ============================================================
// 数据结构
// ============================================================

/// 认证成功后返回的节点信息
struct AuthenticatedNode {
    node_id: String,
    node_name: String,
    role: String,
    virtual_ip: Option<String>,
    routes: Vec<TunnelRoute>,
}

/// 已连接的隧道节点
#[allow(dead_code)]
pub struct TunnelNode {
    /// 节点 ID
    pub node_id: String,
    /// 节点名称
    pub node_name: String,
    /// 角色："client" / "operator" / "server"
    pub role: String,
    /// 虚拟 IP（实施端有值，普通客户端为 None）
    pub virtual_ip: Option<String>,
    /// 隧道写入器（用 Arc<Mutex> 保护，转发时需要写入）
    pub writer: Arc<Mutex<TunnelWriterHalf>>,
    /// 该节点可访问的路由列表
    pub routes: Vec<TunnelRoute>,
    /// 最后活动时间
    pub last_activity: Instant,
}

/// 隧道服务器共享状态
pub struct TunnelState {
    /// node_id -> TunnelNode 的映射
    pub nodes: HashMap<String, TunnelNode>,
    /// 映射网段 CIDR -> node_id 的映射（用于转发查表）
    pub cidr_to_node: HashMap<String, String>,
}

impl TunnelState {
    /// 创建空的隧道状态
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            cidr_to_node: HashMap::new(),
        }
    }
}

impl Default for TunnelState {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// 隧道服务器
// ============================================================

/// 隧道中转服务器
///
/// 监听 TCP 端口，接收客户端和实施端的隧道连接，在它们之间转发 IP 数据包。
pub struct TunnelServer {
    /// 共享隧道状态
    state: Arc<RwLock<TunnelState>>,
    /// 数据库连接池
    db: SqlitePool,
}

impl TunnelServer {
    /// 创建隧道服务器
    pub fn new(db: SqlitePool) -> Self {
        Self {
            state: Arc::new(RwLock::new(TunnelState::new())),
            db,
        }
    }

    /// 获取共享隧道状态（用于 AppState 共享）
    pub fn state(&self) -> Arc<RwLock<TunnelState>> {
        self.state.clone()
    }

    /// 启动隧道服务器，监听 tunnel_addr（如 "0.0.0.0:8444"）
    pub async fn start(&self, tunnel_addr: &str) -> anyhow::Result<()> {
        let listener = TcpListener::bind(tunnel_addr).await?;
        tracing::info!("隧道服务器监听: {}", tunnel_addr);

        // 初始构建路由表
        Self::rebuild_route_table(&self.db, &self.state).await;

        loop {
            match listener.accept().await {
                Ok((stream, peer_addr)) => {
                    let db = self.db.clone();
                    let state = self.state.clone();
                    let peer_addr = peer_addr.to_string();
                    tokio::spawn(async move {
                        Self::handle_connection(db, state, stream, peer_addr).await;
                    });
                }
                Err(e) => {
                    tracing::error!("接受隧道连接失败: {}", e);
                }
            }
        }
    }

    /// 处理单个隧道连接
    async fn handle_connection(
        db: SqlitePool,
        state: Arc<RwLock<TunnelState>>,
        stream: TcpStream,
        peer_addr: String,
    ) {
        tracing::info!("新隧道连接: {}", peer_addr);

        // 分割 TcpStream 为读/写两半
        let (read_half, write_half) = tokio::io::split(stream);
        let mut reader = TunnelReader::new(read_half);
        let mut writer = TunnelWriter::new(write_half);

        // 认证
        let auth_result = match Self::handle_auth(&db, &state, &mut reader, &mut writer).await {
            Ok(Some(result)) => result,
            Ok(None) => {
                tracing::info!("认证失败: {}", peer_addr);
                return;
            }
            Err(e) => {
                tracing::warn!("认证错误 ({}): {}", peer_addr, e);
                return;
            }
        };

        let node_id = auth_result.node_id.clone();
        let node_name = auth_result.node_name.clone();
        let role = auth_result.role.clone();
        let virtual_ip = auth_result.virtual_ip.clone();
        let routes = auth_result.routes.clone();

        tracing::info!(
            "认证成功: {} (node_id={}, role={}, virtual_ip={:?})",
            peer_addr,
            node_id,
            role,
            virtual_ip
        );

        // 如果已有同 ID 节点在线，先移除旧连接
        Self::remove_node(&db, &state, &node_id).await;

        // 将 writer 包装为 Arc<Mutex>，便于转发时跨任务写入
        let writer = Arc::new(Mutex::new(writer));

        // 将节点加入 TunnelState
        {
            let mut st = state.write().await;
            st.nodes.insert(
                node_id.clone(),
                TunnelNode {
                    node_id: node_id.clone(),
                    node_name: node_name.clone(),
                    role: role.clone(),
                    virtual_ip: virtual_ip.clone(),
                    writer: writer.clone(),
                    routes: routes.clone(),
                    last_activity: Instant::now(),
                },
            );
        }

        // 重建路由表并广播给所有在线节点
        Self::rebuild_route_table(&db, &state).await;
        Self::broadcast_route_update(&db, &state).await;

        // 主循环：读取帧并处理
        loop {
            match tokio::time::timeout(Duration::from_secs(90), reader.read_frame()).await {
                Ok(Ok(frame)) => {
                    match frame.frame_type {
                        FrameType::IpPacket => {
                            Self::handle_ip_packet(&state, &frame.payload, &node_id).await;
                        }
                        FrameType::Heartbeat => {
                            Self::handle_heartbeat(&db, &state, &node_id).await;
                        }
                        FrameType::Disconnect => {
                            tracing::info!("节点主动断开: {}", node_id);
                            break;
                        }
                        FrameType::Auth => {
                            tracing::warn!("收到重复认证帧，忽略: {}", node_id);
                        }
                        FrameType::AuthAck | FrameType::RouteUpdate => {
                            tracing::debug!(
                                "服务端不应收到 {} 帧，忽略",
                                frame.frame_type.as_str()
                            );
                        }
                    }
                }
                Ok(Err(e)) => {
                    tracing::warn!("读取帧失败 ({}): {}", node_id, e);
                    break;
                }
                Err(_) => {
                    tracing::warn!("心跳超时 ({}): 90 秒无数据", node_id);
                    break;
                }
            }
        }

        // 连接断开：清理节点并广播
        Self::remove_node(&db, &state, &node_id).await;
        tracing::info!("隧道断开: {} (node_id={})", peer_addr, node_id);
    }

    /// 处理认证帧
    ///
    /// 等待客户端发送 Auth 帧（payload 为 node_id），验证节点是否存在且已启用，
    /// 查询角色、名称、虚拟 IP 和可访问路由，发送 AuthAck 帧。
    ///
    /// 返回 `Ok(Some(...))` 表示认证成功，`Ok(None)` 表示认证失败（已发送失败 Ack），
    /// `Err(...)` 表示 IO 错误。
    async fn handle_auth(
        db: &SqlitePool,
        _state: &Arc<RwLock<TunnelState>>,
        reader: &mut TunnelReaderHalf,
        writer: &mut TunnelWriterHalf,
    ) -> anyhow::Result<Option<AuthenticatedNode>> {
        // 等待认证帧（10 秒超时）
        let frame = match tokio::time::timeout(Duration::from_secs(10), reader.read_frame()).await {
            Ok(Ok(frame)) => frame,
            Ok(Err(e)) => {
                return Err(anyhow::anyhow!("读取认证帧失败: {}", e));
            }
            Err(_) => {
                tracing::warn!("认证超时（10 秒内未收到认证帧）");
                return Ok(None);
            }
        };

        if frame.frame_type != FrameType::Auth {
            // 不是认证帧，发送失败 Ack
            let ack = AuthAckData {
                success: false,
                virtual_ip: None,
                routes: vec![],
                error_message: Some(format!(
                    "期望认证帧，收到 {}",
                    frame.frame_type.as_str()
                )),
            };
            let payload = serde_json::to_vec(&ack).unwrap_or_default();
            let _ = writer.write_frame(&Frame::new(FrameType::AuthAck, payload)).await;
            return Ok(None);
        }

        let node_id = String::from_utf8_lossy(&frame.payload).trim().to_string();
        if node_id.is_empty() {
            let ack = AuthAckData {
                success: false,
                virtual_ip: None,
                routes: vec![],
                error_message: Some("node_id 为空".to_string()),
            };
            let payload = serde_json::to_vec(&ack).unwrap_or_default();
            let _ = writer.write_frame(&Frame::new(FrameType::AuthAck, payload)).await;
            return Ok(None);
        }

        // 查询数据库验证节点
        let row = sqlx::query("SELECT id, name, role, virtual_ip, enabled FROM nodes WHERE id = ?")
            .bind(&node_id)
            .fetch_optional(db)
            .await?;

        let row = match row {
            Some(row) => row,
            None => {
                let ack = AuthAckData {
                    success: false,
                    virtual_ip: None,
                    routes: vec![],
                    error_message: Some(format!("节点不存在: {}", node_id)),
                };
                let payload = serde_json::to_vec(&ack).unwrap_or_default();
                let _ = writer.write_frame(&Frame::new(FrameType::AuthAck, payload)).await;
                return Ok(None);
            }
        };

        let name: String = row.get("name");
        let role_str: String = row.get("role");
        let virtual_ip: Option<String> = row.get("virtual_ip");
        let enabled: i64 = row.get("enabled");

        if enabled == 0 {
            let ack = AuthAckData {
                success: false,
                virtual_ip: None,
                routes: vec![],
                error_message: Some("节点已禁用".to_string()),
            };
            let payload = serde_json::to_vec(&ack).unwrap_or_default();
            let _ = writer.write_frame(&Frame::new(FrameType::AuthAck, payload)).await;
            return Ok(None);
        }

        // 查询该节点可访问的路由列表
        let routes = Self::query_routes_for_node(db, &node_id)
            .await
            .unwrap_or_default();

        // 发送 AuthAck（成功）
        let ack = AuthAckData {
            success: true,
            virtual_ip: virtual_ip.clone(),
            routes: routes.clone(),
            error_message: None,
        };
        let payload = serde_json::to_vec(&ack).unwrap_or_default();
        if let Err(e) = writer.write_frame(&Frame::new(FrameType::AuthAck, payload)).await {
            return Err(anyhow::anyhow!("发送 AuthAck 失败: {}", e));
        }

        // 更新数据库节点状态
        let now = chrono::Utc::now().to_rfc3339();
        let _ = sqlx::query("UPDATE nodes SET status = ?, last_online = ? WHERE id = ?")
            .bind(NodeStatus::Connected.as_str())
            .bind(&now)
            .bind(&node_id)
            .execute(db)
            .await;

        Ok(Some(AuthenticatedNode {
            node_id,
            node_name: name,
            role: role_str,
            virtual_ip,
            routes,
        }))
    }

    /// 处理 IP 包帧：查表转发到目标节点
    ///
    /// 1. 解析 IP 包，获取目标地址
    /// 2. 遍历 cidr_to_node，找到包含目标 IP 的 CIDR
    /// 3. 如果找到目标节点且目标节点在线，转发 IpPacket 帧
    /// 4. 如果找不到目标节点，丢弃包并记录日志
    async fn handle_ip_packet(
        state: &Arc<RwLock<TunnelState>>,
        packet: &[u8],
        source_node_id: &str,
    ) {
        // 解析 IP 包
        let ip_packet = match IpPacket::parse(packet) {
            Some(p) => p,
            None => {
                tracing::warn!("无法解析 IP 包 (from={})", source_node_id);
                return;
            }
        };

        // 查找目标节点：遍历 cidr_to_node，找到包含目标 IP 的 CIDR
        let target_writer = {
            let st = state.read().await;
            let mut found: Option<Arc<Mutex<TunnelWriterHalf>>> = None;
            for (cidr, node_id) in &st.cidr_to_node {
                // 跳过源节点自身（不回环）
                if node_id == source_node_id {
                    continue;
                }
                // 检查目标 IP 是否在此 CIDR 内
                match net_tool_network::ip::ip_in_cidr(&ip_packet.dst_addr, cidr) {
                    Ok(true) => {
                        // 找到匹配的 CIDR，检查目标节点是否在线
                        if let Some(node) = st.nodes.get(node_id) {
                            found = Some(node.writer.clone());
                            break;
                        }
                    }
                    _ => {}
                }
            }
            found
        };

        match target_writer {
            Some(writer) => {
                let mut w = writer.lock().await;
                let frame = Frame::ip_packet(packet);
                if let Err(e) = w.write_frame(&frame).await {
                    tracing::warn!(
                        "转发 IP 包失败 ({} -> {}): {}",
                        source_node_id,
                        ip_packet.dst_addr,
                        e
                    );
                }
            }
            None => {
                tracing::debug!(
                    "丢弃 IP 包: 无匹配路由 ({} -> {}, proto={})",
                    source_node_id,
                    ip_packet.dst_addr,
                    ip_packet.protocol_name()
                );
            }
        }
    }

    /// 处理心跳帧
    ///
    /// 更新节点最后活动时间和数据库状态。
    async fn handle_heartbeat(
        db: &SqlitePool,
        state: &Arc<RwLock<TunnelState>>,
        node_id: &str,
    ) {
        // 更新内存中的最后活动时间
        {
            let mut st = state.write().await;
            if let Some(node) = st.nodes.get_mut(node_id) {
                node.last_activity = Instant::now();
            }
        }

        // 更新数据库状态
        let now = chrono::Utc::now().to_rfc3339();
        let _ = sqlx::query("UPDATE nodes SET status = ?, last_online = ? WHERE id = ?")
            .bind(NodeStatus::Connected.as_str())
            .bind(&now)
            .bind(node_id)
            .execute(db)
            .await;

        tracing::trace!("心跳更新: {}", node_id);
    }

    /// 从数据库加载路由表，构建 cidr_to_node 映射
    ///
    /// 查询所有 status='active' 的网段，构建映射：
    /// mapped_cidr -> node_id（拥有该网段的客户端）
    pub async fn rebuild_route_table(db: &SqlitePool, state: &Arc<RwLock<TunnelState>>) {
        match sqlx::query("SELECT mapped_cidr, node_id FROM segments WHERE status = 'active'")
            .fetch_all(db)
            .await
        {
            Ok(rows) => {
                let mut cidr_to_node = HashMap::new();
                for row in &rows {
                    let mapped_cidr: String = row.get("mapped_cidr");
                    let node_id: String = row.get("node_id");
                    cidr_to_node.insert(mapped_cidr, node_id);
                }

                let count = cidr_to_node.len();
                let mut st = state.write().await;
                st.cidr_to_node = cidr_to_node;
                tracing::info!("路由表已重建: {} 条路由", count);
            }
            Err(e) => {
                tracing::error!("重建路由表失败: {}", e);
            }
        }
    }

    /// 向指定节点发送路由更新
    ///
    /// 查询该节点可访问的路由列表，序列化为 JSON，发送 RouteUpdate 帧。
    pub async fn send_route_update(
        db: &SqlitePool,
        state: &Arc<RwLock<TunnelState>>,
        node_id: &str,
    ) {
        // 查询该节点可访问的路由
        let routes = Self::query_routes_for_node(db, node_id)
            .await
            .unwrap_or_default();
        let routes_json = serde_json::to_string(&routes).unwrap_or_else(|_| "[]".to_string());

        // 获取节点 writer
        let writer = {
            let st = state.read().await;
            st.nodes.get(node_id).map(|n| n.writer.clone())
        };

        if let Some(writer) = writer {
            let mut w = writer.lock().await;
            if let Err(e) = w.write_frame(&Frame::route_update(&routes_json)).await {
                tracing::warn!("发送路由更新失败 ({}): {}", node_id, e);
            } else {
                // 更新节点路由缓存
                let mut st = state.write().await;
                if let Some(node) = st.nodes.get_mut(node_id) {
                    node.routes = routes;
                }
                tracing::debug!("路由更新已发送: {}", node_id);
            }
        }
    }

    /// 向所有在线节点广播路由更新
    pub async fn broadcast_route_update(db: &SqlitePool, state: &Arc<RwLock<TunnelState>>) {
        let node_ids: Vec<String> = {
            let st = state.read().await;
            st.nodes.keys().cloned().collect()
        };

        tracing::info!("广播路由更新给 {} 个在线节点", node_ids.len());

        for node_id in &node_ids {
            Self::send_route_update(db, state, node_id).await;
        }
    }

    /// 节点断开时清理
    ///
    /// 1. 从 TunnelState.nodes 移除节点
    /// 2. 更新数据库状态为 disconnected
    /// 3. 广播路由更新给剩余在线节点
    pub async fn remove_node(db: &SqlitePool, state: &Arc<RwLock<TunnelState>>, node_id: &str) {
        // 从内存状态中移除
        {
            let mut st = state.write().await;
            if st.nodes.remove(node_id).is_none() {
                // 节点不存在（可能已被新连接替换）
                return;
            }
        }

        // 更新数据库状态
        let _ = sqlx::query("UPDATE nodes SET status = ? WHERE id = ?")
            .bind(NodeStatus::Disconnected.as_str())
            .bind(node_id)
            .execute(db)
            .await;

        tracing::info!("节点已移除: {}", node_id);

        // 广播路由更新给剩余在线节点
        Self::broadcast_route_update(db, state).await;
    }

    /// 查询节点的可访问路由列表
    ///
    /// 复用 `handlers::policy::query_routes_for_node` 的权限查询逻辑，
    /// 将 `RouteEntry` 转换为 `TunnelRoute`。
    async fn query_routes_for_node(
        db: &SqlitePool,
        node_id: &str,
    ) -> anyhow::Result<Vec<TunnelRoute>> {
        match crate::handlers::policy::query_routes_for_node(db, node_id).await {
            Ok((_access_mode, routes)) => {
                let tunnel_routes = routes
                    .into_iter()
                    .map(|r| TunnelRoute {
                        mapped_cidr: r.mapped_cidr,
                        target_node_id: r.target_node_id,
                        target_node_name: r.target_node_name,
                        real_cidr: r.real_cidr,
                        segment_name: r.segment_name,
                    })
                    .collect();
                Ok(tunnel_routes)
            }
            Err(e) => Err(anyhow::anyhow!("查询路由失败: {}", e)),
        }
    }
}
