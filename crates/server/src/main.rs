mod config;
mod db;
mod handlers;
mod routes;
mod state;
mod tunnel;

use config::ServerConfig;
use state::AppState;
use tunnel::TunnelServer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 加载配置
    let config = ServerConfig::load();

    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&config.log_level)),
        )
        .init();

    tracing::info!("=== NetTool 服务端启动 ===");
    tracing::info!("监听地址: {}", config.listen_addr);
    tracing::info!("隧道地址: {}", config.tunnel_addr);
    tracing::info!("数据库: {}", config.database_url);
    tracing::info!("Web 管理目录: {}", config.web_admin_dir);
    tracing::info!(
        "客户映射地址池: {}, 实施端地址池: {}, 服务端虚拟 IP: {}",
        config.client_pool_cidr,
        config.operator_pool_cidr,
        config.server_virtual_ip
    );

    // 初始化数据库
    let db = db::init_database(&config.database_url, &config).await?;
    tracing::info!("数据库初始化完成");

    // 创建隧道服务器（内部创建共享隧道状态）
    let tunnel_server = TunnelServer::new(db.clone());
    let tunnel_state = tunnel_server.state();

    // 创建应用状态
    let state = AppState {
        db,
        tunnel_state,
    };

    // 构建路由
    let app = routes::build_router(state, &config.web_admin_dir);

    // 启动隧道服务器（在独立任务中运行）
    let tunnel_addr = config.tunnel_addr.clone();
    tokio::spawn(async move {
        if let Err(e) = tunnel_server.start(&tunnel_addr).await {
            tracing::error!("隧道服务器错误: {}", e);
        }
    });

    // 启动 HTTP 服务
    let listener = tokio::net::TcpListener::bind(&config.listen_addr).await?;
    tracing::info!("服务端已启动，监听 {}", config.listen_addr);

    axum::serve(listener, app).await?;

    Ok(())
}
