//! 隧道客户端模块
//!
//! 负责连接服务端隧道端口（默认 8444），完成认证后通过 TUN 虚拟网卡
//! 转发 IP 数据包，实现跨网段三层互通。
//!
//! ## 工作流程
//!
//! 1. TCP 连接到服务端隧道端口
//! 2. 发送 Auth 帧（payload = node_id）
//! 3. 等待 AuthAck 帧，获取虚拟 IP 和路由列表
//! 4. 创建 TUN 虚拟网卡，添加路由
//! 5. 启动双向转发：TUN <-> 隧道
//! 6. 心跳维持（每 30 秒）

use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use tokio::io::split;
use tokio::net::TcpStream;
use tokio::sync::{Mutex, Notify, RwLock};

use net_tool_network::route::RouteManager;
use net_tool_network::tun::{TunInterface, read_packet, write_packet};
use net_tool_network::tunnel::{
    AuthAckData, Frame, FrameType, TunnelReader, TunnelRoute, TunnelWriter,
};

/// 隧道客户端状态
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TunnelStatus {
    /// 已断开
    Disconnected,
    /// 连接中
    Connecting,
    /// 已认证
    Authenticated,
    /// 运行中（TUN 和隧道已建立）
    Running,
    /// 错误
    Error(String),
}

impl TunnelStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Disconnected => "已断开",
            Self::Connecting => "连接中",
            Self::Authenticated => "已认证",
            Self::Running => "运行中",
            Self::Error(_) => "错误",
        }
    }
}

/// 隧道句柄
///
/// 从 [`TunnelClient`] 中提取的轻量引用，可在命令循环中克隆传递，
/// 用于查询隧道状态、路由信息以及请求停止隧道。
///
/// **设计原因**：`TunnelClient::run()` 是阻塞式主循环，运行期间需要独占
/// `&mut self`。如果将 `TunnelClient` 放入 `Mutex` 并在 `run()` 期间持锁，
/// 会导致 `tunnel status` / `tunnel stop` 等命令死锁。`TunnelHandle` 仅持有
/// `Arc` 引用，不阻塞主循环，从而避免死锁。
#[derive(Clone)]
pub struct TunnelHandle {
    /// 连接状态
    pub status: Arc<RwLock<TunnelStatus>>,
    /// 路由列表
    pub routes: Arc<RwLock<Vec<TunnelRoute>>>,
    /// 虚拟 IP
    pub virtual_ip: Option<String>,
    /// 停止信号（通知 run() 主循环退出）
    stop_notify: Arc<Notify>,
    /// 隧道写入器（用于发送断开帧）
    writer: Arc<Mutex<Option<TunnelWriter<tokio::io::WriteHalf<TcpStream>>>>>,
}

impl TunnelHandle {
    /// 获取当前隧道状态
    pub async fn get_status(&self) -> TunnelStatus {
        self.status.read().await.clone()
    }

    /// 获取当前路由列表
    pub async fn get_routes(&self) -> Vec<TunnelRoute> {
        self.routes.read().await.clone()
    }

    /// 请求停止隧道
    ///
    /// 发送 Disconnect 帧通知服务端，然后触发停止信号让 `run()` 主循环退出。
    /// 实际的资源清理（TUN 网卡、路由）由 `run()` 退出时的 `cleanup()` 完成。
    pub async fn stop(&self) {
        tracing::info!("正在停止隧道...");
        // 发送断开帧
        {
            let mut guard = self.writer.lock().await;
            if let Some(ref mut w) = *guard {
                let frame = Frame::disconnect("客户端主动断开");
                let _ = w.write_frame(&frame).await;
            }
            *guard = None;
        }
        // 通知主循环退出
        self.stop_notify.notify_waiters();
    }
}

/// 隧道客户端
///
/// 负责管理到服务端的隧道连接、TUN 虚拟网卡和路由。
pub struct TunnelClient {
    /// 节点 ID
    pub node_id: String,
    /// 服务端隧道地址（如 "127.0.0.1:8444"）
    pub server_tunnel_addr: String,
    /// 虚拟 IP（实施端有值，普通客户端可能为 None）
    pub virtual_ip: Option<String>,
    /// 可访问的路由列表
    pub routes: Arc<RwLock<Vec<TunnelRoute>>>,
    /// TUN 虚拟网卡名称
    pub tun_name: String,
    /// 连接状态
    pub status: Arc<RwLock<TunnelStatus>>,
    /// 隧道写入器（用于发送数据到服务端）
    writer: Arc<Mutex<Option<TunnelWriter<tokio::io::WriteHalf<TcpStream>>>>>,
    /// 隧道读取器（认证后保存，run() 时取出）
    reader: Option<TunnelReader<tokio::io::ReadHalf<TcpStream>>>,
    /// 路由管理器
    route_manager: Arc<Mutex<RouteManager>>,
    /// TUN 网卡（保持引用以防止提前释放）
    tun_interface: Arc<Mutex<Option<TunInterface>>>,
    /// 是否为实施端
    is_operator: bool,
    /// 停止信号（通知 run() 主循环退出）
    stop_notify: Arc<Notify>,
}

impl TunnelClient {
    /// 创建隧道客户端
    pub fn new(node_id: String, server_tunnel_addr: String, is_operator: bool) -> Self {
        Self {
            node_id,
            server_tunnel_addr,
            virtual_ip: None,
            routes: Arc::new(RwLock::new(Vec::new())),
            tun_name: "nettool0".to_string(),
            status: Arc::new(RwLock::new(TunnelStatus::Disconnected)),
            writer: Arc::new(Mutex::new(None)),
            reader: None,
            route_manager: Arc::new(Mutex::new(RouteManager::new())),
            tun_interface: Arc::new(Mutex::new(None)),
            is_operator,
            stop_notify: Arc::new(Notify::new()),
        }
    }

    /// 提取隧道句柄
    ///
    /// 返回一个轻量的 [`TunnelHandle`]，可在命令循环中使用，
    /// 不会阻塞 `run()` 主循环。
    pub fn handle(&self) -> TunnelHandle {
        TunnelHandle {
            status: self.status.clone(),
            routes: self.routes.clone(),
            virtual_ip: self.virtual_ip.clone(),
            stop_notify: self.stop_notify.clone(),
            writer: self.writer.clone(),
        }
    }

    /// 连接服务端并认证
    ///
    /// 返回认证结果（虚拟 IP、路由列表）。
    pub async fn connect(&mut self) -> Result<AuthAckData> {
        *self.status.write().await = TunnelStatus::Connecting;

        tracing::info!(
            addr = %self.server_tunnel_addr,
            "正在连接服务端隧道"
        );

        // TCP 连接
        let stream = TcpStream::connect(&self.server_tunnel_addr)
            .await
            .with_context(|| format!("连接服务端隧道失败: {}", self.server_tunnel_addr))?;

        // 设置 TCP keepalive
        let _ = stream.set_nodelay(true);

        // 分割为读/写两半
        let (reader_half, writer_half) = split(stream);
        let mut reader = TunnelReader::new(reader_half);
        let mut writer = TunnelWriter::new(writer_half);

        // 发送认证帧
        let auth_frame = Frame::auth(&self.node_id);
        writer
            .write_frame(&auth_frame)
            .await
            .context("发送认证帧失败")?;

        tracing::info!(node_id = %self.node_id, "已发送认证帧");

        // 等待认证回复（10 秒超时）
        let ack_frame = tokio::time::timeout(Duration::from_secs(10), reader.read_frame())
            .await
            .context("等待认证回复超时")?
            .context("读取认证回复失败")?;

        if ack_frame.frame_type != FrameType::AuthAck {
            bail!(
                "期望 AuthAck 帧，收到 {:?}",
                ack_frame.frame_type
            );
        }

        // 解析认证回复
        let ack_data: AuthAckData = serde_json::from_slice(&ack_frame.payload)
            .context("解析认证回复失败")?;

        if !ack_data.success {
            bail!(
                "认证失败: {}",
                ack_data.error_message.unwrap_or_else(|| "未知错误".to_string())
            );
        }

        tracing::info!(
            virtual_ip = ?ack_data.virtual_ip,
            route_count = ack_data.routes.len(),
            "认证成功"
        );

        // 保存状态
        self.virtual_ip = ack_data.virtual_ip.clone();
        *self.routes.write().await = ack_data.routes.clone();
        *self.status.write().await = TunnelStatus::Authenticated;

        // 保存 writer
        *self.writer.lock().await = Some(writer);

        // 将 reader 保存到内部状态，供 run() 使用
        // 这里我们需要把 reader 保存起来，但 TunnelClient 的字段中没有 reader
        // 我们用一种方式：将 reader 放入一个临时存储
        // 实际上，run() 方法需要重新获取 reader，所以我们改用另一种设计：
        // connect() 返回 reader，run() 接收 reader
        // 但这样 API 不太友好。我们改为在 connect 中不保存 reader，
        // 而是在 run 中重新连接。
        //
        // 更好的方案：connect 完成认证后，将 reader 存入一个 oneshot channel 或 Arc<Mutex<Option>>
        // 但 reader 不是 Clone。
        //
        // 最简方案：connect 返回 (AuthAckData, TunnelReader)，run 接收 reader
        // 但这改变了 API。
        //
        // 我们采用：将 reader 存入结构体的一个字段
        // 但 TunnelReader 不是 Send + Sync（除非 ReadHalf 是 Send）
        // tokio::io::ReadHalf<TcpStream> 是 Send
        // 所以我们可以用 Arc<Mutex<Option<TunnelReader>>>
        // 但这样 run() 需要 lock，不太方便。
        //
        // 最终方案：connect 返回 AuthAckData，内部保存 reader 到 self.reader
        // run() 从 self.reader 取出使用

        // 保存 reader 到内部
        self.reader = Some(reader);

        Ok(ack_data)
    }

    /// 启动隧道主循环（认证成功后调用）
    ///
    /// 创建 TUN 网卡、添加路由、开始双向转发。
    /// 此方法会阻塞直到隧道断开。
    pub async fn run(&mut self) -> Result<()> {
        // 取出 reader
        let mut reader = self
            .reader
            .take()
            .context("隧道未认证或 reader 已被取走")?;

        // 确定 TUN 网卡 IP
        let tun_ip = if self.is_operator {
            self.virtual_ip
                .clone()
                .context("实施端缺少虚拟 IP")?
        } else {
            // 普通客户端：使用第一个路由的映射网段的第一个 IP
            let routes = self.routes.read().await;
            if routes.is_empty() {
                bail!("普通客户端没有可用的映射网段");
            }
            // 从 mapped_cidr 提取网络地址，将最后一位改为 1
            let mapped_cidr = &routes[0].mapped_cidr;
            extract_first_ip(mapped_cidr)?
        };

        // 创建 TUN 虚拟网卡
        tracing::info!(name = %self.tun_name, ip = %tun_ip, "正在创建 TUN 虚拟网卡");
        let mut tun = TunInterface::create(&self.tun_name, &tun_ip, 24)
            .context("创建 TUN 虚拟网卡失败")?;
        let mut tun_dev = tun
            .get_async_reader()
            .context("获取 TUN 异步设备失败")?;

        // 保存 TUN 引用
        *self.tun_interface.lock().await = Some(tun);

        // 添加路由：将所有映射网段指向 TUN 网卡
        {
            let routes = self.routes.read().await;
            let mut rm = self.route_manager.lock().await;
            for route in routes.iter() {
                tracing::info!(
                    cidr = %route.mapped_cidr,
                    iface = %self.tun_name,
                    "添加路由"
                );
                if let Err(e) = rm.add_route(&route.mapped_cidr, &self.tun_name) {
                    tracing::warn!(error = %e, cidr = %route.mapped_cidr, "添加路由失败");
                }
            }
        }

        *self.status.write().await = TunnelStatus::Running;
        tracing::info!("隧道已启动，开始转发数据");

        // 获取 writer 的 Arc 引用
        let writer = self.writer.clone();
        let routes = self.routes.clone();
        let status = self.status.clone();

        // 启动心跳任务
        let heartbeat_writer = writer.clone();
        let heartbeat_status = status.clone();
        let heartbeat_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            interval.tick().await; // 跳过第一次立即触发
            loop {
                interval.tick().await;
                let mut guard = heartbeat_writer.lock().await;
                if let Some(ref mut w) = *guard {
                    let frame = Frame::heartbeat();
                    if let Err(e) = w.write_frame(&frame).await {
                        tracing::warn!(error = %e, "发送心跳失败");
                        *heartbeat_status.write().await = TunnelStatus::Error(e.to_string());
                        break;
                    }
                    tracing::trace!("心跳已发送");
                } else {
                    tracing::warn!("隧道 writer 不存在，停止心跳");
                    break;
                }
            }
        });

        // 主循环：使用 select! 同时处理 TUN 读写和隧道读写
        // 由于 tun_rs::AsyncDevice 不实现 AsyncRead/AsyncWrite，
        // 不能用 tokio::io::split 分割，改用 select! 单循环
        let mut tun_buf = vec![0u8; 65535];
        let tunnel_routes = routes.clone();
        let stop_notify = self.stop_notify.clone();

        loop {
            tokio::select! {
                // 优先检查停止信号
                biased;
                _ = stop_notify.notified() => {
                    tracing::info!("收到停止信号，退出隧道主循环");
                    break;
                }
                // 从 TUN 读取 IP 包，转发到隧道
                tun_result = read_packet(&mut tun_dev, &mut tun_buf) => {
                    match tun_result {
                        Ok(n) if n > 0 => {
                            let frame = Frame::ip_packet(&tun_buf[..n]);
                            let mut guard = writer.lock().await;
                            if let Some(ref mut w) = *guard {
                                if let Err(e) = w.write_frame(&frame).await {
                                    tracing::warn!(error = %e, "TUN->隧道 写入失败");
                                    *status.write().await = TunnelStatus::Error(e.to_string());
                                    break;
                                }
                            } else {
                                tracing::warn!("隧道 writer 不存在");
                                break;
                            }
                        }
                        Ok(_) => continue,
                        Err(e) => {
                            tracing::warn!(error = %e, "TUN 读取失败");
                            *status.write().await = TunnelStatus::Error(e.to_string());
                            break;
                        }
                    }
                }
                // 从隧道读取帧，转发到 TUN 或处理控制帧
                tunnel_result = reader.read_frame() => {
                    match tunnel_result {
                        Ok(frame) => match frame.frame_type {
                            FrameType::IpPacket => {
                                if let Err(e) = write_packet(&mut tun_dev, &frame.payload).await {
                                    tracing::warn!(error = %e, "隧道->TUN 写入失败");
                                    *status.write().await = TunnelStatus::Error(e.to_string());
                                    break;
                                }
                            }
                            FrameType::RouteUpdate => {
                                match serde_json::from_slice::<Vec<TunnelRoute>>(&frame.payload) {
                                    Ok(new_routes) => {
                                        tracing::info!(count = new_routes.len(), "收到路由更新");
                                        *tunnel_routes.write().await = new_routes;
                                    }
                                    Err(e) => {
                                        tracing::warn!(error = %e, "解析路由更新失败");
                                    }
                                }
                            }
                            FrameType::Heartbeat => {
                                tracing::trace!("收到服务端心跳回复");
                            }
                            FrameType::Disconnect => {
                                tracing::info!("服务端要求断开连接");
                                *status.write().await = TunnelStatus::Disconnected;
                                break;
                            }
                            _ => {
                                tracing::debug!(frame_type = ?frame.frame_type, "忽略未知帧类型");
                            }
                        },
                        Err(e) => {
                            tracing::warn!(error = %e, "隧道读取失败");
                            *status.write().await = TunnelStatus::Error(e.to_string());
                            break;
                        }
                    }
                }
            }
        }

        // 清理
        heartbeat_handle.abort();
        *self.status.write().await = TunnelStatus::Disconnected;

        // 清理路由和 TUN
        self.cleanup().await;

        Ok(())
    }

    /// 断开连接，清理 TUN 和路由
    ///
    /// 注意：在命令循环中应使用 [`TunnelHandle::stop`] 代替此方法。
    /// 此方法仅在有 `&mut self` 直接访问时使用（如 `run()` 返回后的清理）。
    #[allow(dead_code)]
    pub async fn disconnect(&mut self) -> Result<()> {
        tracing::info!("正在断开隧道连接");

        // 发送断开帧
        {
            let mut guard = self.writer.lock().await;
            if let Some(ref mut w) = *guard {
                let frame = Frame::disconnect("客户端主动断开");
                let _ = w.write_frame(&frame).await;
            }
        }

        // 清理 writer
        *self.writer.lock().await = None;

        // 清理路由和 TUN
        self.cleanup().await;

        *self.status.write().await = TunnelStatus::Disconnected;

        Ok(())
    }

    /// 是否已连接
    #[allow(dead_code)]
    pub async fn is_connected(&self) -> bool {
        let status = self.status.read().await;
        matches!(*status, TunnelStatus::Running | TunnelStatus::Authenticated)
    }

    /// 获取当前状态
    #[allow(dead_code)]
    pub async fn get_status(&self) -> TunnelStatus {
        self.status.read().await.clone()
    }

    /// 获取当前路由列表
    #[allow(dead_code)]
    pub async fn get_routes(&self) -> Vec<TunnelRoute> {
        self.routes.read().await.clone()
    }

    /// 清理路由和 TUN 网卡
    async fn cleanup(&self) {
        // 清理路由
        {
            let mut rm = self.route_manager.lock().await;
            if let Err(e) = rm.cleanup() {
                tracing::warn!(error = %e, "清理路由失败");
            }
        }

        // 关闭 TUN
        {
            let mut tun_guard = self.tun_interface.lock().await;
            if let Some(ref mut tun) = *tun_guard {
                if let Err(e) = tun.close() {
                    tracing::warn!(error = %e, "关闭 TUN 网卡失败");
                }
            }
            *tun_guard = None;
        }

        tracing::info!("隧道资源清理完成");
    }
}

/// 从 CIDR（如 "100.64.1.0/24"）提取第一个可用 IP（如 "100.64.1.1"）
fn extract_first_ip(cidr: &str) -> Result<String> {
    let parts: Vec<&str> = cidr.split('/').collect();
    if parts.len() != 2 {
        bail!("CIDR 格式错误: {}", cidr);
    }
    let ip_parts: Vec<&str> = parts[0].split('.').collect();
    if ip_parts.len() != 4 {
        bail!("IP 格式错误: {}", parts[0]);
    }
    let last_octet: u8 = ip_parts[3]
        .parse()
        .with_context(|| format!("IP 最后一段解析失败: {}", ip_parts[3]))?;
    Ok(format!(
        "{}.{}.{}.{}",
        ip_parts[0],
        ip_parts[1],
        ip_parts[2],
        last_octet + 1
    ))
}

/// 从服务端 REST API 地址推导隧道地址
///
/// REST API 在 8443 端口，隧道在 8444 端口。
/// 支持以下输入格式：
/// - "host:8443" -> "host:8444"
/// - "http://host:8443" -> "host:8444"
/// - "https://host:8443" -> "host:8444"
/// - "host" (无端口) -> "host:8444"
pub fn get_tunnel_addr(server_addr: &str) -> String {
    // 去除协议前缀
    let addr = server_addr
        .strip_prefix("https://")
        .or_else(|| server_addr.strip_prefix("http://"))
        .unwrap_or(server_addr);

    // 去除路径部分
    let addr = addr.split('/').next().unwrap_or(addr);

    // 分割 host 和 port
    if let Some(colon_pos) = addr.rfind(':') {
        let host = &addr[..colon_pos];
        format!("{}:8444", host)
    } else {
        format!("{}:8444", addr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_first_ip() {
        assert_eq!(extract_first_ip("100.64.1.0/24").unwrap(), "100.64.1.1");
        assert_eq!(extract_first_ip("100.127.0.0/24").unwrap(), "100.127.0.1");
    }

    #[test]
    fn test_get_tunnel_addr() {
        assert_eq!(get_tunnel_addr("host:8443"), "host:8444");
        assert_eq!(get_tunnel_addr("http://host:8443"), "host:8444");
        assert_eq!(get_tunnel_addr("https://host:8443"), "host:8444");
        assert_eq!(get_tunnel_addr("host"), "host:8444");
        assert_eq!(get_tunnel_addr("192.168.1.1:8443"), "192.168.1.1:8444");
    }
}
