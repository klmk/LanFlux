use std::sync::Arc;

use sqlx::SqlitePool;
use tokio::sync::RwLock;

use crate::tunnel::TunnelState;

/// 应用共享状态
///
/// 通过 axum 的 State extractor 在各 handler 之间共享。
/// SqlitePool 内部是 Arc 包装的，clone 开销很低。
/// tunnel_state 同样是 Arc 包装的，clone 开销很低。
#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    /// 隧道服务器共享状态（与 TunnelServer 共享同一个 Arc）
    pub tunnel_state: Arc<RwLock<TunnelState>>,
}
