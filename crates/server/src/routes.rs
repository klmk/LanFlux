use axum::routing::{get, post, put};
use axum::Router;
use tower_http::services::ServeDir;

use crate::handlers::{dashboard, node, policy, pool, segment, session, tunnel};
use crate::state::AppState;

/// 构建 API 路由
///
/// 路由前缀 /api/v1，包含节点、网段、权限、地址池、会话、仪表盘等接口。
/// 同时提供静态文件服务，映射 / 到 web_admin_dir。
pub fn build_router(state: AppState, web_admin_dir: &str) -> Router {
    // API v1 路由
    let api_v1 = Router::new()
        // ---- 节点管理 ----
        .route("/nodes/register", post(node::register))
        .route("/nodes/heartbeat", post(node::heartbeat))
        .route("/nodes", get(node::list))
        .route("/nodes/:id", get(node::get).put(node::update))
        .route("/nodes/:id/disable", post(node::disable))
        .route("/nodes/:id/enable", post(node::enable))
        .route("/nodes/:id/kick", post(node::kick))
        // ---- 网段管理 ----
        .route("/segments/report", post(segment::report))
        .route("/segments", get(segment::list))
        .route("/segments/:id", get(segment::get).put(segment::update))
        .route("/segments/:id/enable", post(segment::enable))
        .route("/segments/:id/disable", post(segment::disable))
        .route("/segments/:id/remap", post(segment::remap))
        // ---- 映射关系 ----
        .route("/mappings", get(segment::mappings))
        // ---- 权限管理 ----
        .route("/policies", get(policy::list))
        .route("/policies/:node_id", put(policy::update))
        .route("/policies/:node_id/detail", get(policy::get_by_node))
        .route("/access", get(policy::query_access))
        .route("/access/query", post(policy::query_access_post))
        // ---- 实施端管理 ----
        .route("/operators/request-ip", post(pool::request_operator_ip))
        // ---- 地址池配置 ----
        .route("/pools", get(pool::get).put(pool::update))
        // ---- 连接会话 ----
        .route("/sessions", get(session::list))
        // ---- 仪表盘 ----
        .route("/dashboard", get(dashboard::dashboard))
        // ---- 日志 ----
        .route("/logs", get(dashboard::logs))
        // ---- 隧道管理 ----
        .route("/tunnel/rebuild", post(tunnel::rebuild))
        .with_state(state);

    // 主路由：API + 静态文件服务
    Router::new()
        .nest("/api/v1", api_v1)
        .fallback_service(ServeDir::new(web_admin_dir))
}
