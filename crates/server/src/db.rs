use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

use crate::config::ServerConfig;

/// 初始化数据库
///
/// 创建连接池，执行建表语句，并初始化地址池默认配置。
pub async fn init_database(database_url: &str, config: &ServerConfig) -> anyhow::Result<SqlitePool> {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await?;

    create_tables(&pool).await?;
    init_address_pool(&pool, config).await?;

    Ok(pool)
}

/// 创建所有数据表（如果不存在）
async fn create_tables(pool: &SqlitePool) -> anyhow::Result<()> {
    // 节点表
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS nodes (
            id                      TEXT PRIMARY KEY,
            name                    TEXT NOT NULL,
            role                    TEXT NOT NULL,
            status                  TEXT NOT NULL DEFAULT 'disconnected',
            os_type                 TEXT NOT NULL DEFAULT 'linux',
            virtual_ip              TEXT,
            reported_segments_count INTEGER NOT NULL DEFAULT 0,
            last_online             TEXT,
            created_at              TEXT NOT NULL,
            remark                  TEXT NOT NULL DEFAULT '',
            enabled                 INTEGER NOT NULL DEFAULT 1
        )
        "#,
    )
    .execute(pool)
    .await?;

    // 网段表
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS segments (
            id          TEXT PRIMARY KEY,
            node_id     TEXT NOT NULL,
            name        TEXT NOT NULL,
            real_cidr   TEXT NOT NULL,
            mapped_cidr TEXT NOT NULL,
            status      TEXT NOT NULL DEFAULT 'active',
            remark      TEXT NOT NULL DEFAULT '',
            created_at  TEXT NOT NULL,
            updated_at  TEXT NOT NULL,
            FOREIGN KEY (node_id) REFERENCES nodes(id)
        )
        "#,
    )
    .execute(pool)
    .await?;

    // 权限策略表
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS access_policies (
            id               TEXT PRIMARY KEY,
            node_id          TEXT NOT NULL UNIQUE,
            access_mode      TEXT NOT NULL DEFAULT 'denied',
            allowed_segments TEXT NOT NULL DEFAULT '[]',
            created_at       TEXT NOT NULL,
            updated_at       TEXT NOT NULL,
            FOREIGN KEY (node_id) REFERENCES nodes(id)
        )
        "#,
    )
    .execute(pool)
    .await?;

    // 地址池配置表
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS address_pools (
            id                          INTEGER PRIMARY KEY,
            client_pool_cidr            TEXT NOT NULL,
            segment_size                INTEGER NOT NULL,
            operator_pool_cidr          TEXT NOT NULL,
            server_virtual_ip           TEXT NOT NULL,
            next_client_segment_index   INTEGER NOT NULL DEFAULT 1,
            next_operator_ip_index      INTEGER NOT NULL DEFAULT 2
        )
        "#,
    )
    .execute(pool)
    .await?;

    // 连接会话表
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS connection_sessions (
            id               TEXT PRIMARY KEY,
            source_node_id   TEXT NOT NULL,
            target_segment_id TEXT NOT NULL,
            protocol         TEXT NOT NULL,
            target_address   TEXT NOT NULL,
            target_client_id TEXT NOT NULL,
            started_at       TEXT NOT NULL,
            last_activity    TEXT NOT NULL,
            status           TEXT NOT NULL DEFAULT 'active',
            FOREIGN KEY (source_node_id) REFERENCES nodes(id),
            FOREIGN KEY (target_segment_id) REFERENCES segments(id),
            FOREIGN KEY (target_client_id) REFERENCES nodes(id)
        )
        "#,
    )
    .execute(pool)
    .await?;

    // 创建索引
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_segments_node_id ON segments(node_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_segments_status ON segments(status)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_access_policies_node_id ON access_policies(node_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_sessions_status ON connection_sessions(status)")
        .execute(pool)
        .await?;

    tracing::info!("数据库表初始化完成");
    Ok(())
}

/// 初始化地址池配置
///
/// 如果地址池表为空，则插入一条默认记录（id=1）。
/// 默认值来自 ServerConfig。
async fn init_address_pool(pool: &SqlitePool, config: &ServerConfig) -> anyhow::Result<()> {
    let row: Option<(i64,)> = sqlx::query_as("SELECT id FROM address_pools WHERE id = 1")
        .fetch_optional(pool)
        .await?;

    if row.is_none() {
        sqlx::query(
            r#"
            INSERT INTO address_pools (
                id, client_pool_cidr, segment_size, operator_pool_cidr,
                server_virtual_ip, next_client_segment_index, next_operator_ip_index
            ) VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(1i64)
        .bind(&config.client_pool_cidr)
        .bind(config.segment_size as i32)
        .bind(&config.operator_pool_cidr)
        .bind(&config.server_virtual_ip)
        .bind(1i64) // next_client_segment_index 从 1 开始
        .bind(2i64) // next_operator_ip_index 从 2 开始
        .execute(pool)
        .await?;

        tracing::info!("地址池配置已初始化");
    }

    Ok(())
}
