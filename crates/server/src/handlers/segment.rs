use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use sqlx::Row;

use net_tool_common::{
    ApiResponse, MappingInfo, ReportSegmentRequest, ReportSegmentResponse, SegmentInfo,
    SegmentStatus, UpdateSegmentRequest,
};

use crate::state::AppState;

// ============================================================
// 辅助函数
// ============================================================

/// 验证 CIDR 格式（只允许 /24）
fn validate_cidr_24(cidr: &str) -> bool {
    if !cidr.ends_with("/24") {
        return false;
    }
    let ip_part = &cidr[..cidr.len() - 3];
    let parts: Vec<&str> = ip_part.split('.').collect();
    if parts.len() != 4 {
        return false;
    }
    for part in parts {
        match part.parse::<u8>() {
            Ok(_) => {}
            Err(_) => return false,
        }
    }
    true
}

/// 生成访问示例
///
/// 例如：真实网段 192.168.1.0/24，映射网段 100.64.1.0/24
/// 生成 "192.168.1.10 -> 100.64.1.10"
fn generate_access_example(real_cidr: &str, mapped_cidr: &str) -> String {
    let real_base = real_cidr.split('/').next().unwrap_or("");
    let mapped_base = mapped_cidr.split('/').next().unwrap_or("");

    let real_parts: Vec<&str> = real_base.split('.').collect();
    let mapped_parts: Vec<&str> = mapped_base.split('.').collect();

    if real_parts.len() == 4 && mapped_parts.len() == 4 {
        format!(
            "{}.{}.{}.10 -> {}.{}.{}.10",
            real_parts[0], real_parts[1], real_parts[2],
            mapped_parts[0], mapped_parts[1], mapped_parts[2]
        )
    } else {
        format!("{} -> {}", real_cidr, mapped_cidr)
    }
}

fn row_to_segment_info(row: &sqlx::sqlite::SqliteRow) -> SegmentInfo {
    let status_str: String = row.get("status");
    SegmentInfo {
        id: row.get("id"),
        node_id: row.get("node_id"),
        name: row.get("name"),
        real_cidr: row.get("real_cidr"),
        mapped_cidr: row.get("mapped_cidr"),
        status: SegmentStatus::from_db_str(&status_str),
        remark: row.get("remark"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

// ============================================================
// HTTP Handler
// ============================================================

/// 上报网段
///
/// 接收 ReportSegmentRequest，验证 CIDR 格式（只允许 /24），
/// 分配映射网段（从 100.64.1.0/24 开始递增），
/// 存入数据库，返回 ReportSegmentResponse。
pub async fn report(
    State(state): State<AppState>,
    Json(req): Json<ReportSegmentRequest>,
) -> Json<ApiResponse<ReportSegmentResponse>> {
    // 验证 CIDR 格式
    if !validate_cidr_24(&req.real_cidr) {
        return Json(ApiResponse::err(format!(
            "网段格式无效，初版只支持 /24 网段: {}",
            req.real_cidr
        )));
    }

    // 分配映射网段
    let mapped_cidr = match super::pool::allocate_client_segment(&state.db).await {
        Ok(cidr) => cidr,
        Err(e) => return Json(ApiResponse::err(format!("分配映射网段失败: {}", e))),
    };

    let segment_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let remark = req.remark.clone().unwrap_or_default();

    let result = sqlx::query(
        r#"INSERT INTO segments
           (id, node_id, name, real_cidr, mapped_cidr, status, remark, created_at, updated_at)
           VALUES (?, ?, ?, ?, ?, 'active', ?, ?, ?)"#,
    )
    .bind(&segment_id)
    .bind(&req.node_id)
    .bind(&req.name)
    .bind(&req.real_cidr)
    .bind(&mapped_cidr)
    .bind(&remark)
    .bind(&now)
    .bind(&now)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            // 更新节点的上报网段计数
            let _ = sqlx::query(
                "UPDATE nodes SET reported_segments_count = reported_segments_count + 1 WHERE id = ?",
            )
            .bind(&req.node_id)
            .execute(&state.db)
            .await;

            tracing::info!(
                "网段上报成功: id={}, node_id={}, real_cidr={}, mapped_cidr={}",
                segment_id,
                req.node_id,
                req.real_cidr,
                mapped_cidr
            );

            Json(ApiResponse::ok(ReportSegmentResponse {
                segment_id,
                mapped_cidr,
            }))
        }
        Err(e) => Json(ApiResponse::err(format!("网段上报失败: {}", e))),
    }
}

/// 网段列表查询参数
#[derive(Debug, Deserialize)]
pub struct SegmentListQuery {
    /// 按节点 ID 筛选
    pub node_id: Option<String>,
}

/// 查询所有网段，支持按 node_id 筛选
pub async fn list(
    State(state): State<AppState>,
    Query(query): Query<SegmentListQuery>,
) -> Json<ApiResponse<Vec<SegmentInfo>>> {
    let result = if let Some(node_id) = &query.node_id {
        sqlx::query("SELECT * FROM segments WHERE node_id = ? ORDER BY created_at DESC")
            .bind(node_id)
            .fetch_all(&state.db)
            .await
    } else {
        sqlx::query("SELECT * FROM segments ORDER BY created_at DESC")
            .fetch_all(&state.db)
            .await
    };

    match result {
        Ok(rows) => {
            let segments: Vec<SegmentInfo> = rows.iter().map(row_to_segment_info).collect();
            Json(ApiResponse::ok(segments))
        }
        Err(e) => Json(ApiResponse::err(format!("查询网段列表失败: {}", e))),
    }
}

/// 查询单个网段详情
pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<SegmentInfo>> {
    match sqlx::query("SELECT * FROM segments WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.db)
        .await
    {
        Ok(Some(row)) => Json(ApiResponse::ok(row_to_segment_info(&row))),
        Ok(None) => Json(ApiResponse::err(format!("网段 {} 不存在", id))),
        Err(e) => Json(ApiResponse::err(format!("查询网段失败: {}", e))),
    }
}

/// 修改网段名称和备注
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateSegmentRequest>,
) -> Json<ApiResponse<SegmentInfo>> {
    let now = chrono::Utc::now().to_rfc3339();

    let result = sqlx::query(
        r#"UPDATE segments SET
           name       = COALESCE(?, name),
           remark     = COALESCE(?, remark),
           updated_at = ?
           WHERE id = ?"#,
    )
    .bind(req.name.as_deref())
    .bind(req.remark.as_deref())
    .bind(&now)
    .bind(&id)
    .execute(&state.db)
    .await;

    match result {
        Ok(res) => {
            if res.rows_affected() == 0 {
                return Json(ApiResponse::err(format!("网段 {} 不存在", id)));
            }
            match sqlx::query("SELECT * FROM segments WHERE id = ?")
                .bind(&id)
                .fetch_one(&state.db)
                .await
            {
                Ok(row) => Json(ApiResponse::ok(row_to_segment_info(&row))),
                Err(e) => Json(ApiResponse::err(format!("获取更新后的网段失败: {}", e))),
            }
        }
        Err(e) => Json(ApiResponse::err(format!("更新网段失败: {}", e))),
    }
}

/// 启用网段
pub async fn enable(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<SegmentInfo>> {
    let now = chrono::Utc::now().to_rfc3339();

    match sqlx::query("UPDATE segments SET status = 'active', updated_at = ? WHERE id = ?")
        .bind(&now)
        .bind(&id)
        .execute(&state.db)
        .await
    {
        Ok(res) => {
            if res.rows_affected() == 0 {
                return Json(ApiResponse::err(format!("网段 {} 不存在", id)));
            }
            match sqlx::query("SELECT * FROM segments WHERE id = ?")
                .bind(&id)
                .fetch_one(&state.db)
                .await
            {
                Ok(row) => Json(ApiResponse::ok(row_to_segment_info(&row))),
                Err(e) => Json(ApiResponse::err(format!("获取网段失败: {}", e))),
            }
        }
        Err(e) => Json(ApiResponse::err(format!("启用网段失败: {}", e))),
    }
}

/// 停用网段
pub async fn disable(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<SegmentInfo>> {
    let now = chrono::Utc::now().to_rfc3339();

    match sqlx::query("UPDATE segments SET status = 'disabled', updated_at = ? WHERE id = ?")
        .bind(&now)
        .bind(&id)
        .execute(&state.db)
        .await
    {
        Ok(res) => {
            if res.rows_affected() == 0 {
                return Json(ApiResponse::err(format!("网段 {} 不存在", id)));
            }
            match sqlx::query("SELECT * FROM segments WHERE id = ?")
                .bind(&id)
                .fetch_one(&state.db)
                .await
            {
                Ok(row) => Json(ApiResponse::ok(row_to_segment_info(&row))),
                Err(e) => Json(ApiResponse::err(format!("获取网段失败: {}", e))),
            }
        }
        Err(e) => Json(ApiResponse::err(format!("停用网段失败: {}", e))),
    }
}

/// 重新分配映射网段
///
/// 为指定网段重新分配一个映射网段。
/// 这是一个高影响操作，已有访问地址会变化。
pub async fn remap(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<SegmentInfo>> {
    // 分配新的映射网段
    let new_mapped_cidr = match super::pool::allocate_client_segment(&state.db).await {
        Ok(cidr) => cidr,
        Err(e) => return Json(ApiResponse::err(format!("分配映射网段失败: {}", e))),
    };

    let now = chrono::Utc::now().to_rfc3339();

    let result = sqlx::query(
        "UPDATE segments SET mapped_cidr = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&new_mapped_cidr)
    .bind(&now)
    .bind(&id)
    .execute(&state.db)
    .await;

    match result {
        Ok(res) => {
            if res.rows_affected() == 0 {
                return Json(ApiResponse::err(format!("网段 {} 不存在", id)));
            }
            tracing::info!("网段 {} 重新分配映射网段: {}", id, new_mapped_cidr);
            match sqlx::query("SELECT * FROM segments WHERE id = ?")
                .bind(&id)
                .fetch_one(&state.db)
                .await
            {
                Ok(row) => Json(ApiResponse::ok(row_to_segment_info(&row))),
                Err(e) => Json(ApiResponse::err(format!("获取网段失败: {}", e))),
            }
        }
        Err(e) => Json(ApiResponse::err(format!("重新分配映射网段失败: {}", e))),
    }
}

/// 映射关系列表
///
/// 展示真实网段和平台映射网段的对照关系。
pub async fn mappings(State(state): State<AppState>) -> Json<ApiResponse<Vec<MappingInfo>>> {
    match sqlx::query(
        r#"SELECT s.id, s.node_id, s.name, s.real_cidr, s.mapped_cidr, s.status,
                  COALESCE(n.name, '') AS node_name
           FROM segments s
           LEFT JOIN nodes n ON s.node_id = n.id
           ORDER BY s.created_at DESC"#,
    )
    .fetch_all(&state.db)
    .await
    {
        Ok(rows) => {
            let mappings: Vec<MappingInfo> = rows
                .iter()
                .map(|row| {
                    let real_cidr: String = row.get("real_cidr");
                    let mapped_cidr: String = row.get("mapped_cidr");
                    let status_str: String = row.get("status");

                    MappingInfo {
                        segment_id: row.get("id"),
                        node_id: row.get("node_id"),
                        node_name: row.get("node_name"),
                        segment_name: row.get("name"),
                        access_example: generate_access_example(&real_cidr, &mapped_cidr),
                        real_cidr,
                        mapped_cidr,
                        status: SegmentStatus::from_db_str(&status_str),
                    }
                })
                .collect();
            Json(ApiResponse::ok(mappings))
        }
        Err(e) => Json(ApiResponse::err(format!("查询映射关系失败: {}", e))),
    }
}
