use axum::extract::{Path, State};
use axum::Json;
use sqlx::Row;

use net_tool_common::{
    ApiResponse, HeartbeatRequest, HeartbeatResponse, NodeInfo, NodeRole, NodeStatus, OsType,
    RegisterRequest, RegisterResponse, UpdateNodeRequest,
};

use crate::state::AppState;

// ============================================================
// 行转换辅助
// ============================================================

fn row_to_node_info(row: &sqlx::sqlite::SqliteRow) -> NodeInfo {
    let role_str: String = row.get("role");
    let status_str: String = row.get("status");
    let os_str: String = row.get("os_type");
    let enabled: i64 = row.get("enabled");

    NodeInfo {
        id: row.get("id"),
        name: row.get("name"),
        role: NodeRole::from_db_str(&role_str),
        status: NodeStatus::from_db_str(&status_str),
        os_type: OsType::from_db_str(&os_str),
        virtual_ip: row.get("virtual_ip"),
        reported_segments_count: row.get("reported_segments_count"),
        last_online: row.get("last_online"),
        created_at: row.get("created_at"),
        remark: row.get("remark"),
        enabled: enabled != 0,
    }
}

// ============================================================
// HTTP Handler
// ============================================================

/// 节点注册
///
/// 接收 RegisterRequest，生成 UUID。
/// 如果是 Operator 角色则分配虚拟 IP（从 100.127.0.2 开始递增）。
/// 存入数据库，返回 RegisterResponse。
pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> Json<ApiResponse<RegisterResponse>> {
    let node_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let remark = req.remark.clone().unwrap_or_default();

    // 如果是实施端，分配虚拟 IP
    let virtual_ip = if req.role == NodeRole::Operator {
        match super::pool::allocate_operator_ip(&state.db).await {
            Ok(ip) => Some(ip),
            Err(e) => {
                return Json(ApiResponse::err(format!("分配实施端 IP 失败: {}", e)));
            }
        }
    } else {
        None
    };

    let result = sqlx::query(
        r#"INSERT INTO nodes
           (id, name, role, status, os_type, virtual_ip, reported_segments_count,
            last_online, created_at, remark, enabled)
           VALUES (?, ?, ?, ?, ?, ?, 0, ?, ?, ?, 1)"#,
    )
    .bind(&node_id)
    .bind(&req.name)
    .bind(req.role.as_str())
    .bind(NodeStatus::Connected.as_str())
    .bind(req.os_type.as_str())
    .bind(virtual_ip.as_deref())
    .bind(&now)
    .bind(&now)
    .bind(&remark)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            tracing::info!(
                "节点注册成功: id={}, name={}, role={}, virtual_ip={:?}",
                node_id,
                req.name,
                req.role.as_str(),
                virtual_ip
            );
            Json(ApiResponse::ok(RegisterResponse {
                node_id,
                virtual_ip,
            }))
        }
        Err(e) => Json(ApiResponse::err(format!("节点注册失败: {}", e))),
    }
}

/// 心跳
///
/// 更新节点状态为 Connected 和最后在线时间。
/// 返回当前应下发给该节点的路由列表。
pub async fn heartbeat(
    State(state): State<AppState>,
    Json(req): Json<HeartbeatRequest>,
) -> Json<ApiResponse<HeartbeatResponse>> {
    let now = chrono::Utc::now().to_rfc3339();

    // 更新节点状态和最后在线时间
    let result = if let Some(count) = req.reported_segments_count {
        sqlx::query(
            r#"UPDATE nodes
               SET status = ?, last_online = ?, reported_segments_count = ?
               WHERE id = ?"#,
        )
        .bind(NodeStatus::Connected.as_str())
        .bind(&now)
        .bind(count)
        .bind(&req.node_id)
        .execute(&state.db)
        .await
    } else {
        sqlx::query(
            r#"UPDATE nodes
               SET status = ?, last_online = ?
               WHERE id = ?"#,
        )
        .bind(NodeStatus::Connected.as_str())
        .bind(&now)
        .bind(&req.node_id)
        .execute(&state.db)
        .await
    };

    match result {
        Ok(res) => {
            if res.rows_affected() == 0 {
                return Json(ApiResponse::err(format!(
                    "节点 {} 不存在",
                    req.node_id
                )));
            }

            // 查询应下发的路由
            match super::policy::query_routes_for_node(&state.db, &req.node_id).await {
                Ok((_, routes)) => Json(ApiResponse::ok(HeartbeatResponse { routes })),
                Err(e) => Json(ApiResponse::err(format!("查询路由失败: {}", e))),
            }
        }
        Err(e) => Json(ApiResponse::err(format!("心跳更新失败: {}", e))),
    }
}

/// 查询所有节点
pub async fn list(State(state): State<AppState>) -> Json<ApiResponse<Vec<NodeInfo>>> {
    match sqlx::query("SELECT * FROM nodes ORDER BY created_at DESC")
        .fetch_all(&state.db)
        .await
    {
        Ok(rows) => {
            let nodes: Vec<NodeInfo> = rows.iter().map(row_to_node_info).collect();
            Json(ApiResponse::ok(nodes))
        }
        Err(e) => Json(ApiResponse::err(format!("查询节点列表失败: {}", e))),
    }
}

/// 查询单个节点详情
pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<NodeInfo>> {
    match sqlx::query("SELECT * FROM nodes WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.db)
        .await
    {
        Ok(Some(row)) => Json(ApiResponse::ok(row_to_node_info(&row))),
        Ok(None) => Json(ApiResponse::err(format!("节点 {} 不存在", id))),
        Err(e) => Json(ApiResponse::err(format!("查询节点失败: {}", e))),
    }
}

/// 修改节点名称和备注
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateNodeRequest>,
) -> Json<ApiResponse<NodeInfo>> {
    let result = sqlx::query(
        r#"UPDATE nodes SET
           name   = COALESCE(?, name),
           remark = COALESCE(?, remark)
           WHERE id = ?"#,
    )
    .bind(req.name.as_deref())
    .bind(req.remark.as_deref())
    .bind(&id)
    .execute(&state.db)
    .await;

    match result {
        Ok(res) => {
            if res.rows_affected() == 0 {
                return Json(ApiResponse::err(format!("节点 {} 不存在", id)));
            }
            match sqlx::query("SELECT * FROM nodes WHERE id = ?")
                .bind(&id)
                .fetch_one(&state.db)
                .await
            {
                Ok(row) => Json(ApiResponse::ok(row_to_node_info(&row))),
                Err(e) => Json(ApiResponse::err(format!("获取更新后的节点失败: {}", e))),
            }
        }
        Err(e) => Json(ApiResponse::err(format!("更新节点失败: {}", e))),
    }
}

/// 禁用节点
pub async fn disable(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<NodeInfo>> {
    match sqlx::query("UPDATE nodes SET enabled = 0 WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await
    {
        Ok(res) => {
            if res.rows_affected() == 0 {
                return Json(ApiResponse::err(format!("节点 {} 不存在", id)));
            }
            match sqlx::query("SELECT * FROM nodes WHERE id = ?")
                .bind(&id)
                .fetch_one(&state.db)
                .await
            {
                Ok(row) => Json(ApiResponse::ok(row_to_node_info(&row))),
                Err(e) => Json(ApiResponse::err(format!("获取节点失败: {}", e))),
            }
        }
        Err(e) => Json(ApiResponse::err(format!("禁用节点失败: {}", e))),
    }
}

/// 启用节点
pub async fn enable(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<NodeInfo>> {
    match sqlx::query("UPDATE nodes SET enabled = 1 WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await
    {
        Ok(res) => {
            if res.rows_affected() == 0 {
                return Json(ApiResponse::err(format!("节点 {} 不存在", id)));
            }
            match sqlx::query("SELECT * FROM nodes WHERE id = ?")
                .bind(&id)
                .fetch_one(&state.db)
                .await
            {
                Ok(row) => Json(ApiResponse::ok(row_to_node_info(&row))),
                Err(e) => Json(ApiResponse::err(format!("获取节点失败: {}", e))),
            }
        }
        Err(e) => Json(ApiResponse::err(format!("启用节点失败: {}", e))),
    }
}

/// 踢下线
///
/// 设置节点状态为 Disconnected。
pub async fn kick(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<NodeInfo>> {
    match sqlx::query("UPDATE nodes SET status = ? WHERE id = ?")
        .bind(NodeStatus::Disconnected.as_str())
        .bind(&id)
        .execute(&state.db)
        .await
    {
        Ok(res) => {
            if res.rows_affected() == 0 {
                return Json(ApiResponse::err(format!("节点 {} 不存在", id)));
            }
            tracing::info!("节点已被踢下线: {}", id);
            match sqlx::query("SELECT * FROM nodes WHERE id = ?")
                .bind(&id)
                .fetch_one(&state.db)
                .await
            {
                Ok(row) => Json(ApiResponse::ok(row_to_node_info(&row))),
                Err(e) => Json(ApiResponse::err(format!("获取节点失败: {}", e))),
            }
        }
        Err(e) => Json(ApiResponse::err(format!("踢下线失败: {}", e))),
    }
}
