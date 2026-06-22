use axum::extract::State;
use axum::Json;
use sqlx::{Row, SqlitePool};

use net_tool_common::{
    AddressPoolInfo, ApiResponse, RequestOperatorIpRequest, RequestOperatorIpResponse,
    UpdatePoolRequest,
};

use crate::state::AppState;

// ============================================================
// 内部分配方法（供其他 handler 调用）
// ============================================================

/// 分配客户映射网段
///
/// 从 next_client_segment_index 递增，生成 100.64.{index}.0/24。
/// - 第 1 次调用 -> 100.64.1.0/24
/// - 第 2 次调用 -> 100.64.2.0/24
/// - 第 N 次调用 -> 100.64.{N}.0/24
///
/// N 从 1 开始递增，最大 254。
pub async fn allocate_client_segment(pool: &SqlitePool) -> Result<String, String> {
    let row = sqlx::query("SELECT next_client_segment_index FROM address_pools WHERE id = 1")
        .fetch_one(pool)
        .await
        .map_err(|e| format!("查询地址池失败: {}", e))?;

    let index: i64 = row.get("next_client_segment_index");

    if index > 254 {
        return Err("客户映射网段已耗尽（最大 254）".to_string());
    }

    // 递增索引
    sqlx::query("UPDATE address_pools SET next_client_segment_index = ? WHERE id = 1")
        .bind(index + 1)
        .execute(pool)
        .await
        .map_err(|e| format!("更新地址池索引失败: {}", e))?;

    let cidr = format!("100.64.{}.0/24", index);
    tracing::info!("分配客户映射网段: {} (index={})", cidr, index);
    Ok(cidr)
}

/// 分配实施端 IP
///
/// 从 next_operator_ip_index 递增，生成 100.127.0.{index}。
/// - 第 1 次调用 -> 100.127.0.2
/// - 第 2 次调用 -> 100.127.0.3
/// - 第 N 次调用 -> 100.127.0.{N+1}
///
/// 从 2 开始递增，最大 254。
pub async fn allocate_operator_ip(pool: &SqlitePool) -> Result<String, String> {
    let row = sqlx::query("SELECT next_operator_ip_index FROM address_pools WHERE id = 1")
        .fetch_one(pool)
        .await
        .map_err(|e| format!("查询地址池失败: {}", e))?;

    let index: i64 = row.get("next_operator_ip_index");

    if index > 254 {
        return Err("实施端 IP 已耗尽（最大 254）".to_string());
    }

    // 递增索引
    sqlx::query("UPDATE address_pools SET next_operator_ip_index = ? WHERE id = 1")
        .bind(index + 1)
        .execute(pool)
        .await
        .map_err(|e| format!("更新地址池索引失败: {}", e))?;

    let ip = format!("100.127.0.{}", index);
    tracing::info!("分配实施端 IP: {} (index={})", ip, index);
    Ok(ip)
}

// ============================================================
// 行转换辅助
// ============================================================

fn row_to_pool_info(row: &sqlx::sqlite::SqliteRow) -> AddressPoolInfo {
    AddressPoolInfo {
        id: row.get("id"),
        client_pool_cidr: row.get("client_pool_cidr"),
        segment_size: row.get("segment_size"),
        operator_pool_cidr: row.get("operator_pool_cidr"),
        server_virtual_ip: row.get("server_virtual_ip"),
        next_client_segment_index: row.get("next_client_segment_index"),
        next_operator_ip_index: row.get("next_operator_ip_index"),
    }
}

// ============================================================
// HTTP Handler
// ============================================================

/// 获取地址池配置
pub async fn get(State(state): State<AppState>) -> Json<ApiResponse<AddressPoolInfo>> {
    match sqlx::query("SELECT * FROM address_pools WHERE id = 1")
        .fetch_one(&state.db)
        .await
    {
        Ok(row) => Json(ApiResponse::ok(row_to_pool_info(&row))),
        Err(e) => Json(ApiResponse::err(format!("获取地址池配置失败: {}", e))),
    }
}

/// 修改地址池配置
pub async fn update(
    State(state): State<AppState>,
    Json(req): Json<UpdatePoolRequest>,
) -> Json<ApiResponse<AddressPoolInfo>> {
    let result = sqlx::query(
        r#"UPDATE address_pools SET
           client_pool_cidr   = COALESCE(?, client_pool_cidr),
           segment_size       = COALESCE(?, segment_size),
           operator_pool_cidr = COALESCE(?, operator_pool_cidr),
           server_virtual_ip  = COALESCE(?, server_virtual_ip)
           WHERE id = 1"#,
    )
    .bind(req.client_pool_cidr.as_deref())
    .bind(req.segment_size)
    .bind(req.operator_pool_cidr.as_deref())
    .bind(req.server_virtual_ip.as_deref())
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            match sqlx::query("SELECT * FROM address_pools WHERE id = 1")
                .fetch_one(&state.db)
                .await
            {
                Ok(row) => Json(ApiResponse::ok(row_to_pool_info(&row))),
                Err(e) => Json(ApiResponse::err(format!("获取地址池配置失败: {}", e))),
            }
        }
        Err(e) => Json(ApiResponse::err(format!("更新地址池配置失败: {}", e))),
    }
}

/// 实施端申请虚拟 IP 与可访问网段
///
/// 接收 `RequestOperatorIpRequest`，返回已分配的虚拟 IP 和可访问网段列表。
/// 实施端的虚拟 IP 在注册时已分配，此接口主要用于获取可访问网段。
pub async fn request_operator_ip(
    State(state): State<AppState>,
    Json(req): Json<RequestOperatorIpRequest>,
) -> Json<ApiResponse<RequestOperatorIpResponse>> {
    // 查询节点，获取虚拟 IP
    let node_row = sqlx::query("SELECT virtual_ip, role FROM nodes WHERE id = ?")
        .bind(&req.node_id)
        .fetch_one(&state.db)
        .await;

    match node_row {
        Ok(row) => {
            let virtual_ip: Option<String> = row.get("virtual_ip");
            let role_str: String = row.get("role");

            let virtual_ip = match virtual_ip {
                Some(ip) => ip,
                None => {
                    // 如果没有虚拟 IP（可能注册时未分配），尝试分配
                    match allocate_operator_ip(&state.db).await {
                        Ok(ip) => {
                            // 更新节点记录
                            let _ = sqlx::query("UPDATE nodes SET virtual_ip = ? WHERE id = ?")
                                .bind(&ip)
                                .bind(&req.node_id)
                                .execute(&state.db)
                                .await;
                            ip
                        }
                        Err(e) => {
                            return Json(ApiResponse::err(format!(
                                "分配实施端 IP 失败: {}",
                                e
                            )));
                        }
                    }
                }
            };

            // 获取可访问网段
            let (access_mode, routes) =
                match super::policy::query_routes_for_node(&state.db, &req.node_id).await {
                    Ok(v) => v,
                    Err(e) => {
                        return Json(ApiResponse::err(format!("查询可访问网段失败: {}", e)));
                    }
                };

            tracing::info!(
                "实施端 {} 申请 IP: {} (角色={}, 访问模式={}, 网段数={})",
                req.node_id,
                virtual_ip,
                role_str,
                access_mode.as_str(),
                routes.len()
            );

            Json(ApiResponse::ok(RequestOperatorIpResponse {
                virtual_ip,
                accessible_segments: routes,
            }))
        }
        Err(e) => Json(ApiResponse::err(format!("节点不存在: {}", e))),
    }
}
