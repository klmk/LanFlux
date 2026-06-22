use axum::extract::State;
use axum::Json;

use net_tool_common::ApiResponse;

use crate::state::AppState;
use crate::tunnel::TunnelServer;

/// 重建路由表并广播给所有在线节点
///
/// POST /api/v1/tunnel/rebuild
///
/// 从数据库重新加载所有 active 网段，重建 cidr_to_node 映射，
/// 然后向所有在线隧道节点广播路由更新。
pub async fn rebuild(State(state): State<AppState>) -> Json<ApiResponse<String>> {
    tracing::info!("收到路由表重建请求");

    TunnelServer::rebuild_route_table(&state.db, &state.tunnel_state).await;
    TunnelServer::broadcast_route_update(&state.db, &state.tunnel_state).await;

    let online_count = {
        let st = state.tunnel_state.read().await;
        st.nodes.len()
    };

    tracing::info!("路由表重建并广播完成，当前在线节点: {}", online_count);

    Json(ApiResponse::ok(format!(
        "路由表已重建并广播给 {} 个在线节点",
        online_count
    )))
}
