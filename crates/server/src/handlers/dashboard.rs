use axum::extract::State;
use axum::Json;
use sqlx::Row;

use net_tool_common::{ApiResponse, DashboardData, LogEntry};

use crate::state::AppState;

/// 仪表盘数据
///
/// 返回：
/// - 在线节点数
/// - 普通客户端数
/// - 实施端数
/// - 已登记网段数
/// - 已启用映射网段数
/// - 异常网段数
/// - 当前连接会话数
pub async fn dashboard(State(state): State<AppState>) -> Json<ApiResponse<DashboardData>> {
    // 在线节点数（状态为 connected 或 networking，且已启用）
    let online_node_count = count_scalar(
        &state.db,
        "SELECT COUNT(*) FROM nodes WHERE status IN ('connected', 'networking') AND enabled = 1",
    )
    .await;

    // 普通客户端数
    let client_count =
        count_scalar(&state.db, "SELECT COUNT(*) FROM nodes WHERE role = 'client'").await;

    // 实施端数
    let operator_count =
        count_scalar(&state.db, "SELECT COUNT(*) FROM nodes WHERE role = 'operator'").await;

    // 已登记网段数
    let registered_segment_count =
        count_scalar(&state.db, "SELECT COUNT(*) FROM segments").await;

    // 已启用映射网段数
    let active_mapped_segment_count =
        count_scalar(&state.db, "SELECT COUNT(*) FROM segments WHERE status = 'active'").await;

    // 异常网段数
    let error_segment_count =
        count_scalar(&state.db, "SELECT COUNT(*) FROM segments WHERE status = 'error'").await;

    // 当前连接会话数
    let active_session_count =
        count_scalar(&state.db, "SELECT COUNT(*) FROM connection_sessions WHERE status = 'active'")
            .await;

    Json(ApiResponse::ok(DashboardData {
        online_node_count,
        client_count,
        operator_count,
        registered_segment_count,
        active_mapped_segment_count,
        error_segment_count,
        active_session_count,
    }))
}

/// 执行 COUNT 查询并返回 i64
async fn count_scalar(pool: &sqlx::SqlitePool, sql: &str) -> i64 {
    match sqlx::query(sql).fetch_one(pool).await {
        Ok(row) => {
            let count: i64 = row.get(0);
            count
        }
        Err(e) => {
            tracing::warn!("COUNT 查询失败: {}, SQL: {}", e, sql);
            0
        }
    }
}

/// 日志列表
///
/// 当前版本暂未实现日志存储，返回空列表。
/// 后续可通过 tracing 日志文件或专用日志表实现。
pub async fn logs() -> Json<ApiResponse<Vec<LogEntry>>> {
    // TODO: 实现日志存储和查询
    Json(ApiResponse::ok(vec![]))
}
