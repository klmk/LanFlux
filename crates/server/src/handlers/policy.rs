use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use sqlx::{Row, SqlitePool};

use net_tool_common::{
    AccessMode, AccessPolicyInfo, AccessQueryResponse, ApiResponse, QueryAccessRequest,
    QueryAccessResponse, RouteEntry, UpdatePolicyRequest,
};

use crate::state::AppState;

// ============================================================
// 内部辅助方法（供其他 handler 调用）
// ============================================================

/// 查询指定节点应下发的路由列表
///
/// 权限查询逻辑：
/// - Operator 角色：返回 AllowedAll，可访问所有 Active 状态的网段
/// - Client 角色：查询 access_policies 表，如果没有记录则返回 Denied（空列表），
///   如果有记录则按配置返回
/// - Server 角色：返回 AllowedAll（空路由列表，服务端本身不需要路由）
pub async fn query_routes_for_node(
    pool: &SqlitePool,
    node_id: &str,
) -> Result<(AccessMode, Vec<RouteEntry>), String> {
    // 查询节点角色
    let row = sqlx::query("SELECT role FROM nodes WHERE id = ?")
        .bind(node_id)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("节点不存在: {}", e))?;

    let role_str: String = row.get("role");
    let role = net_tool_common::NodeRole::from_db_str(&role_str);

    match role {
        net_tool_common::NodeRole::Operator => {
            // 实施端：返回所有 Active 网段
            let routes = query_active_segment_routes(pool, None).await?;
            Ok((AccessMode::AllowedAll, routes))
        }
        net_tool_common::NodeRole::Server => {
            // 服务端：AllowedAll，但不需要路由
            Ok((AccessMode::AllowedAll, vec![]))
        }
        net_tool_common::NodeRole::Client => {
            // 普通客户端：查询权限策略
            let policy_row = sqlx::query(
                "SELECT access_mode, allowed_segments FROM access_policies WHERE node_id = ?",
            )
            .bind(node_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;

            match policy_row {
                None => {
                    // 没有权限记录，返回 Denied
                    Ok((AccessMode::Denied, vec![]))
                }
                Some(row) => {
                    let access_mode_str: String = row.get("access_mode");
                    let access_mode = AccessMode::from_db_str(&access_mode_str);
                    let allowed_segments_str: String = row.get("allowed_segments");
                    let allowed_segments: Vec<String> =
                        serde_json::from_str(&allowed_segments_str).unwrap_or_default();

                    let routes = match access_mode {
                        AccessMode::AllowedAll => {
                            // 允许访问全部网段
                            query_active_segment_routes(pool, None).await?
                        }
                        AccessMode::AllowedSegments => {
                            // 只允许访问指定网段
                            query_active_segment_routes(pool, Some(&allowed_segments)).await?
                        }
                        AccessMode::Denied => vec![],
                    };

                    Ok((access_mode, routes))
                }
            }
        }
    }
}

/// 查询所有 Active 状态的网段路由
///
/// 如果提供了 allowed_segment_ids，则只返回 ID 在列表中的网段。
async fn query_active_segment_routes(
    pool: &SqlitePool,
    allowed_segment_ids: Option<&[String]>,
) -> Result<Vec<RouteEntry>, String> {
    let rows = sqlx::query(
        r#"SELECT s.id, s.name, s.mapped_cidr, s.node_id, s.real_cidr,
                  COALESCE(n.name, '') AS node_name
           FROM segments s
           LEFT JOIN nodes n ON s.node_id = n.id
           WHERE s.status = 'active'"#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut routes = Vec::new();
    for row in &rows {
        let segment_id: String = row.get("id");

        // 如果指定了允许的网段列表，进行过滤
        if let Some(allowed) = allowed_segment_ids {
            if !allowed.contains(&segment_id) {
                continue;
            }
        }

        let mapped_cidr: String = row.get("mapped_cidr");
        let target_node_id: String = row.get("node_id");
        let target_node_name: String = row.get("node_name");
        let real_cidr: String = row.get("real_cidr");
        let segment_name: String = row.get("name");

        routes.push(RouteEntry {
            mapped_cidr,
            target_node_id,
            target_node_name,
            real_cidr,
            segment_name,
        });
    }

    Ok(routes)
}

// ============================================================
// 行转换辅助
// ============================================================

fn row_to_policy_info(row: &sqlx::sqlite::SqliteRow) -> AccessPolicyInfo {
    let access_mode_str: String = row.get("access_mode");
    let allowed_segments_str: String = row.get("allowed_segments");
    let allowed_segments: Vec<String> =
        serde_json::from_str(&allowed_segments_str).unwrap_or_default();

    AccessPolicyInfo {
        id: row.get("id"),
        node_id: row.get("node_id"),
        access_mode: AccessMode::from_db_str(&access_mode_str),
        allowed_segments,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

// ============================================================
// HTTP Handler
// ============================================================

/// 查询所有权限策略
pub async fn list(State(state): State<AppState>) -> Json<ApiResponse<Vec<AccessPolicyInfo>>> {
    match sqlx::query("SELECT * FROM access_policies ORDER BY updated_at DESC")
        .fetch_all(&state.db)
        .await
    {
        Ok(rows) => {
            let policies: Vec<AccessPolicyInfo> =
                rows.iter().map(row_to_policy_info).collect();
            Json(ApiResponse::ok(policies))
        }
        Err(e) => Json(ApiResponse::err(format!("查询权限策略失败: {}", e))),
    }
}

/// 查询指定节点的权限
pub async fn get_by_node(
    State(state): State<AppState>,
    Path(node_id): Path<String>,
) -> Json<ApiResponse<AccessPolicyInfo>> {
    match sqlx::query("SELECT * FROM access_policies WHERE node_id = ?")
        .bind(&node_id)
        .fetch_optional(&state.db)
        .await
    {
        Ok(Some(row)) => Json(ApiResponse::ok(row_to_policy_info(&row))),
        Ok(None) => Json(ApiResponse::err(format!(
            "节点 {} 没有权限策略配置",
            node_id
        ))),
        Err(e) => Json(ApiResponse::err(format!("查询权限策略失败: {}", e))),
    }
}

/// 修改节点权限
pub async fn update(
    State(state): State<AppState>,
    Path(node_id): Path<String>,
    Json(req): Json<UpdatePolicyRequest>,
) -> Json<ApiResponse<AccessPolicyInfo>> {
    let now = chrono::Utc::now().to_rfc3339();
    let allowed_segments_json = serde_json::to_string(&req.allowed_segments)
        .unwrap_or_else(|_| "[]".to_string());

    // 先尝试更新
    let result = sqlx::query(
        r#"UPDATE access_policies
           SET access_mode = ?, allowed_segments = ?, updated_at = ?
           WHERE node_id = ?"#,
    )
    .bind(req.access_mode.as_str())
    .bind(&allowed_segments_json)
    .bind(&now)
    .bind(&node_id)
    .execute(&state.db)
    .await;

    match result {
        Ok(res) => {
            if res.rows_affected() == 0 {
                // 不存在，创建新记录
                let policy_id = uuid::Uuid::new_v4().to_string();
                if let Err(e) = sqlx::query(
                    r#"INSERT INTO access_policies
                       (id, node_id, access_mode, allowed_segments, created_at, updated_at)
                       VALUES (?, ?, ?, ?, ?, ?)"#,
                )
                .bind(&policy_id)
                .bind(&node_id)
                .bind(req.access_mode.as_str())
                .bind(&allowed_segments_json)
                .bind(&now)
                .bind(&now)
                .execute(&state.db)
                .await
                {
                    return Json(ApiResponse::err(format!("创建权限策略失败: {}", e)));
                }
                tracing::info!("创建权限策略: node_id={}, mode={}", node_id, req.access_mode.as_str());
            } else {
                tracing::info!("更新权限策略: node_id={}, mode={}", node_id, req.access_mode.as_str());
            }

            // 返回更新后的策略
            match sqlx::query("SELECT * FROM access_policies WHERE node_id = ?")
                .bind(&node_id)
                .fetch_one(&state.db)
                .await
            {
                Ok(row) => Json(ApiResponse::ok(row_to_policy_info(&row))),
                Err(e) => Json(ApiResponse::err(format!("获取权限策略失败: {}", e))),
            }
        }
        Err(e) => Json(ApiResponse::err(format!("更新权限策略失败: {}", e))),
    }
}

/// 客户端/实施端查询自己的访问权限
#[derive(Debug, Deserialize)]
pub struct AccessQueryParams {
    pub node_id: String,
}

pub async fn query_access(
    State(state): State<AppState>,
    Query(params): Query<AccessQueryParams>,
) -> Json<ApiResponse<AccessQueryResponse>> {
    match query_routes_for_node(&state.db, &params.node_id).await {
        Ok((access_mode, routes)) => Json(ApiResponse::ok(AccessQueryResponse {
            node_id: params.node_id,
            access_mode,
            routes,
        })),
        Err(e) => Json(ApiResponse::err(e)),
    }
}

/// 查询访问权限（POST 版本，接受 JSON body）
///
/// 供客户端/实施端通过 POST 请求查询可访问网段。
/// 返回 `QueryAccessResponse`（与客户端期望的格式一致）。
pub async fn query_access_post(
    State(state): State<AppState>,
    Json(req): Json<QueryAccessRequest>,
) -> Json<ApiResponse<QueryAccessResponse>> {
    match query_routes_for_node(&state.db, &req.node_id).await {
        Ok((access_mode, routes)) => Json(ApiResponse::ok(QueryAccessResponse {
            access_mode,
            allowed_segments: routes,
        })),
        Err(e) => Json(ApiResponse::err(e)),
    }
}
