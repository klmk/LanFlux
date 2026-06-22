//! 服务端连接管理
//!
//! 封装与组网服务端的所有 HTTP 交互：
//! - 节点注册 `POST /api/v1/nodes/register`
//! - 心跳维持 `POST /api/v1/nodes/heartbeat`（每 30 秒）
//! - 网段上报 `POST /api/v1/segments/report`
//! - 网段查询 `POST /api/v1/segments/query`
//! - 网段删除 `DELETE /api/v1/segments/{id}`
//! - 实施端地址申请 `POST /api/v1/operators/request-ip`
//! - 权限查询 `POST /api/v1/access/query`
//!
//! 连接失败时按指数退避重试。所有响应统一解析为
//! [`net_tool_common::ApiResponse<T>`]。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use net_tool_common::{
    ApiResponse, HeartbeatResponse, QueryAccessResponse, QuerySegmentsResponse,
    RegisterRequest, RegisterResponse, ReportSegmentRequest, ReportSegmentResponse,
    RequestOperatorIpResponse, SegmentSummary,
};
use tokio::sync::Mutex;

/// 连接状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionStatus {
    /// 已断开
    Disconnected,
    /// 连接中
    Connecting,
    /// 已连接
    Connected,
    /// 重连中
    Reconnecting,
    /// 连接失败
    Failed,
}

impl ConnectionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Disconnected => "已断开",
            Self::Connecting => "连接中",
            Self::Connected => "已连接",
            Self::Reconnecting => "重连中",
            Self::Failed => "连接失败",
        }
    }
}

/// 与服务端的连接句柄。
///
/// 内部的 [`reqwest::Client`] 可廉价克隆，因此 [`ServerConnection`] 实现了
/// [`Clone`]，便于在心跳任务与命令循环之间共享。
#[derive(Clone)]
pub struct ServerConnection {
    #[allow(dead_code)]
    client: reqwest::Client,
    /// 原始服务端地址（保留用于诊断展示）
    #[allow(dead_code)]
    server_addr: String,
    base_url: String,
    node_id: Option<String>,
    virtual_ip: Option<String>,
    status: ConnectionStatus,
}

impl ServerConnection {
    /// 创建连接句柄。`server_addr` 可为 `IP:port` 或完整 URL。
    pub fn new(server_addr: &str) -> Result<Self> {
        let base_url = normalize_base_url(server_addr)?;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .connect_timeout(Duration::from_secs(10))
            .no_proxy()
            .build()
            .context("构建 HTTP 客户端失败")?;
        Ok(Self {
            client,
            server_addr: server_addr.to_string(),
            base_url,
            node_id: None,
            virtual_ip: None,
            status: ConnectionStatus::Disconnected,
        })
    }

    #[allow(dead_code)]
    pub fn server_addr(&self) -> &str {
        &self.server_addr
    }
    #[allow(dead_code)]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
    pub fn node_id(&self) -> Option<&str> {
        self.node_id.as_deref()
    }
    pub fn virtual_ip(&self) -> Option<&str> {
        self.virtual_ip.as_deref()
    }
    pub fn status(&self) -> ConnectionStatus {
        self.status.clone()
    }
    #[allow(dead_code)]
    pub fn set_status(&mut self, s: ConnectionStatus) {
        self.status = s;
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    /// 发送 JSON POST 请求并解析 `ApiResponse<T>`。
    async fn post_json<Req, Resp>(&self, path: &str, body: &Req) -> Result<Resp>
    where
        Req: serde::Serialize,
        Resp: serde::de::DeserializeOwned,
    {
        let mut req = self.client.post(self.url(path)).json(body);
        if let Some(id) = &self.node_id {
            req = req.header("X-Node-Id", id);
        }
        let resp = req.send().await.context("HTTP 请求失败")?;
        let api: ApiResponse<Resp> = resp.json().await.context("解析响应失败")?;
        if api.code != 0 {
            bail!(if api.message.is_empty() {
                "未知错误".to_string()
            } else {
                api.message.clone()
            });
        }
        api.data.ok_or_else(|| anyhow::anyhow!("响应缺少 data 字段"))
    }

    /// 节点注册。
    pub async fn register(&mut self, req: RegisterRequest) -> Result<RegisterResponse> {
        self.status = ConnectionStatus::Connecting;
        let resp: RegisterResponse = self.post_json("/api/v1/nodes/register", &req).await?;
        self.node_id = Some(resp.node_id.clone());
        self.virtual_ip = resp.virtual_ip.clone();
        self.status = ConnectionStatus::Connected;
        Ok(resp)
    }

    /// 心跳。
    pub async fn heartbeat(&self) -> Result<HeartbeatResponse> {
        let req = net_tool_common::HeartbeatRequest {
            node_id: self.node_id.clone().unwrap_or_default(),
            reported_segments_count: None,
        };
        self.post_json("/api/v1/nodes/heartbeat", &req).await
    }

    /// 上报网段。
    ///
    /// `node_id` 由连接内部填充，调用方只需提供名称、真实网段与备注。
    pub async fn report_segment(
        &self,
        name: &str,
        real_cidr: &str,
        remark: Option<String>,
    ) -> Result<ReportSegmentResponse> {
        let req = ReportSegmentRequest {
            node_id: self.node_id.clone().unwrap_or_default(),
            name: name.to_string(),
            real_cidr: real_cidr.to_string(),
            remark,
        };
        self.post_json("/api/v1/segments/report", &req).await
    }

    /// 查询本节点已上报的网段。
    pub async fn query_segments(&self) -> Result<Vec<SegmentSummary>> {
        let req = net_tool_common::QuerySegmentsRequest {
            node_id: self.node_id.clone().unwrap_or_default(),
        };
        let resp: QuerySegmentsResponse = self.post_json("/api/v1/segments/query", &req).await?;
        Ok(resp.segments)
    }

    /// 删除已上报的网段。
    #[allow(dead_code)]
    pub async fn delete_segment(&self, segment_id: &str) -> Result<()> {
        let mut req = self
            .client
            .delete(self.url(&format!("/api/v1/segments/{segment_id}")));
        if let Some(id) = &self.node_id {
            req = req.header("X-Node-Id", id);
        }
        let resp = req.send().await.context("HTTP 请求失败")?;
        let api: ApiResponse<()> = resp.json().await.context("解析响应失败")?;
        if api.code != 0 {
            bail!(if api.message.is_empty() {
                "未知错误".to_string()
            } else {
                api.message.clone()
            });
        }
        Ok(())
    }

    /// 实施端申请虚拟 IP 与可访问网段。
    pub async fn request_operator_ip(&self) -> Result<RequestOperatorIpResponse> {
        let req = net_tool_common::RequestOperatorIpRequest {
            node_id: self.node_id.clone().unwrap_or_default(),
        };
        self.post_json("/api/v1/operators/request-ip", &req).await
    }

    /// 查询访问权限与可访问网段。
    pub async fn query_access(&self) -> Result<QueryAccessResponse> {
        let req = net_tool_common::QueryAccessRequest {
            node_id: self.node_id.clone().unwrap_or_default(),
        };
        self.post_json("/api/v1/access/query", &req).await
    }

    /// 带指数退避的注册重试。
    pub async fn register_with_retry(
        &mut self,
        req: RegisterRequest,
        max_attempts: u32,
    ) -> Result<RegisterResponse> {
        let mut attempt = 0u32;
        loop {
            attempt += 1;
            match self.register(req.clone()).await {
                Ok(resp) => return Ok(resp),
                Err(e) => {
                    if attempt >= max_attempts {
                        self.status = ConnectionStatus::Failed;
                        bail!("注册失败（已重试 {attempt} 次）: {e}");
                    }
                    // 指数退避：1, 2, 4, 8, ... 上限 30 秒
                    let delay = std::cmp::min(30u64, 1u64 << attempt.min(5));
                    self.status = ConnectionStatus::Reconnecting;
                    eprintln!(
                        "[连接] 注册失败，{delay} 秒后重试（第 {attempt}/{max_attempts} 次）: {e}"
                    );
                    tokio::time::sleep(Duration::from_secs(delay)).await;
                }
            }
        }
    }
}

/// 启动心跳循环。
///
/// 每 30 秒发送一次心跳；失败时按指数退避重试，并更新共享状态。
/// 当 `running` 被置为 `false` 时退出循环。
pub async fn run_heartbeat(
    conn: ServerConnection,
    status: Arc<Mutex<ConnectionStatus>>,
    running: Arc<AtomicBool>,
) {
    let mut fail_count = 0u32;
    // 首次延迟 30 秒，避免与注册紧挨。
    while running.load(Ordering::SeqCst) {
        tokio::time::sleep(Duration::from_secs(30)).await;
        if !running.load(Ordering::SeqCst) {
            break;
        }
        match conn.heartbeat().await {
            Ok(_) => {
                let mut s = status.lock().await;
                *s = ConnectionStatus::Connected;
                drop(s);
                fail_count = 0;
            }
            Err(e) => {
                fail_count += 1;
                {
                    let mut s = status.lock().await;
                    *s = ConnectionStatus::Reconnecting;
                }
                let delay = std::cmp::min(60u64, 5u64 * (1u64 << fail_count.min(4)));
                eprintln!(
                    "[心跳] 失败（第 {fail_count} 次）: {e}，{delay} 秒后重试"
                );
                tokio::time::sleep(Duration::from_secs(delay)).await;
            }
        }
    }
    let mut s = status.lock().await;
    *s = ConnectionStatus::Disconnected;
}

/// 把 `IP:port` 或完整 URL 规整为带 scheme 的 base URL。
fn normalize_base_url(server_addr: &str) -> Result<String> {
    let addr = server_addr.trim().trim_end_matches('/');
    if addr.is_empty() {
        bail!("服务端地址为空");
    }
    if addr.starts_with("http://") || addr.starts_with("https://") {
        Ok(addr.to_string())
    } else {
        Ok(format!("http://{addr}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_url() {
        assert_eq!(
            normalize_base_url("127.0.0.1:8443").unwrap(),
            "http://127.0.0.1:8443"
        );
        assert_eq!(
            normalize_base_url("https://a.com:8443/").unwrap(),
            "https://a.com:8443"
        );
    }
}
