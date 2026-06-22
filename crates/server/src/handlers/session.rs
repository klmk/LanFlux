use axum::extract::State;
use axum::Json;
use sqlx::Row;

use net_tool_common::{ApiResponse, ConnectionSessionInfo, Protocol, SessionStatus};

use crate::state::AppState;

// ============================================================
// 行转换辅助
// ============================================================

fn row_to_session_info(row: &sqlx::sqlite::SqliteRow) -> ConnectionSessionInfo {
    let protocol_str: String = row.get("protocol");
    let status_str: String = row.get("status");

    ConnectionSessionInfo {
        id: row.get("id"),
        source_node_id: row.get("source_node_id"),
        target_segment_id: row.get("target_segment_id"),
        protocol: Protocol::from_db_str(&protocol_str),
        target_address: row.get("target_address"),
        target_client_id: row.get("target_client_id"),
        started_at: row.get("started_at"),
        last_activity: row.get("last_activity"),
        status: SessionStatus::from_db_str(&status_str),
    }
}

// ============================================================
// HTTP Handler
// ============================================================

/// 查询连接会话列表
pub async fn list(State(state): State<AppState>) -> Json<ApiResponse<Vec<ConnectionSessionInfo>>> {
    match sqlx::query("SELECT * FROM connection_sessions ORDER BY started_at DESC")
        .fetch_all(&state.db)
        .await
    {
        Ok(rows) => {
            let sessions: Vec<ConnectionSessionInfo> =
                rows.iter().map(row_to_session_info).collect();
            Json(ApiResponse::ok(sessions))
        }
        Err(e) => Json(ApiResponse::err(format!("查询连接会话失败: {}", e))),
    }
}
