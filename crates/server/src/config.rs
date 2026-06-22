use serde::Deserialize;

/// 服务端配置
///
/// 可以从 TOML 配置文件加载，也可以通过环境变量覆盖。
/// 默认配置适用于本地开发和 Docker 部署。
#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    /// HTTP 监听地址
    #[serde(default = "default_listen_addr")]
    pub listen_addr: String,

    /// SQLite 数据库连接字符串
    #[serde(default = "default_database_url")]
    pub database_url: String,

    /// Web 管理后台静态文件目录
    #[serde(default = "default_web_admin_dir")]
    pub web_admin_dir: String,

    /// 日志级别
    #[serde(default = "default_log_level")]
    pub log_level: String,

    /// 客户映射地址池 CIDR
    #[serde(default = "default_client_pool_cidr")]
    pub client_pool_cidr: String,

    /// 单个映射网段大小（前缀长度），初版固定为 24
    #[serde(default = "default_segment_size")]
    pub segment_size: u32,

    /// 实施端地址池 CIDR
    #[serde(default = "default_operator_pool_cidr")]
    pub operator_pool_cidr: String,

    /// 服务端虚拟 IP
    #[serde(default = "default_server_virtual_ip")]
    pub server_virtual_ip: String,

    /// 隧道服务器监听地址
    #[serde(default = "default_tunnel_addr")]
    pub tunnel_addr: String,
}

fn default_listen_addr() -> String {
    "0.0.0.0:8443".to_string()
}

fn default_database_url() -> String {
    "sqlite://data/nettool.db?mode=rwc".to_string()
}

fn default_web_admin_dir() -> String {
    "./web-admin".to_string()
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_client_pool_cidr() -> String {
    "100.64.0.0/10".to_string()
}

fn default_segment_size() -> u32 {
    24
}

fn default_operator_pool_cidr() -> String {
    "100.127.0.0/24".to_string()
}

fn default_server_virtual_ip() -> String {
    "100.127.0.1".to_string()
}

fn default_tunnel_addr() -> String {
    "0.0.0.0:8444".to_string()
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            listen_addr: default_listen_addr(),
            database_url: default_database_url(),
            web_admin_dir: default_web_admin_dir(),
            log_level: default_log_level(),
            client_pool_cidr: default_client_pool_cidr(),
            segment_size: default_segment_size(),
            operator_pool_cidr: default_operator_pool_cidr(),
            server_virtual_ip: default_server_virtual_ip(),
            tunnel_addr: default_tunnel_addr(),
        }
    }
}

impl ServerConfig {
    /// 加载配置
    ///
    /// 优先级：环境变量 > TOML 配置文件 > 默认值
    pub fn load() -> Self {
        let mut config = Self::load_from_file().unwrap_or_default();
        Self::override_from_env(&mut config);
        config
    }

    /// 从 TOML 文件加载配置
    ///
    /// 按以下顺序查找配置文件：
    /// 1. 环境变量 NETTOOL_CONFIG 指定的路径
    /// 2. 当前目录下的 config.toml
    /// 3. /etc/nettool/config.toml
    fn load_from_file() -> Option<Self> {
        let candidates: Vec<String> = match std::env::var("NETTOOL_CONFIG") {
            Ok(path) => vec![path],
            Err(_) => vec![
                "config.toml".to_string(),
                "/etc/nettool/config.toml".to_string(),
            ],
        };

        for path in &candidates {
            if let Ok(content) = std::fs::read_to_string(path) {
                match toml::from_str::<ServerConfig>(&content) {
                    Ok(config) => {
                        tracing::info!("从 {} 加载配置", path);
                        return Some(config);
                    }
                    Err(e) => {
                        tracing::warn!("解析配置文件 {} 失败: {}", path, e);
                    }
                }
            }
        }

        None
    }

    /// 使用环境变量覆盖配置
    fn override_from_env(config: &mut Self) {
        if let Ok(v) = std::env::var("NETTOOL_LISTEN_ADDR") {
            config.listen_addr = v;
        }
        if let Ok(v) = std::env::var("NETTOOL_DATABASE_URL") {
            config.database_url = v;
        }
        if let Ok(v) = std::env::var("NETTOOL_WEB_ADMIN_DIR") {
            config.web_admin_dir = v;
        }
        if let Ok(v) = std::env::var("NETTOOL_LOG_LEVEL") {
            config.log_level = v;
        }
        if let Ok(v) = std::env::var("NETTOOL_CLIENT_POOL_CIDR") {
            config.client_pool_cidr = v;
        }
        if let Ok(v) = std::env::var("NETTOOL_SEGMENT_SIZE") {
            if let Ok(size) = v.parse::<u32>() {
                config.segment_size = size;
            }
        }
        if let Ok(v) = std::env::var("NETTOOL_OPERATOR_POOL_CIDR") {
            config.operator_pool_cidr = v;
        }
        if let Ok(v) = std::env::var("NETTOOL_SERVER_VIRTUAL_IP") {
            config.server_virtual_ip = v;
        }
        if let Ok(v) = std::env::var("NETTOOL_TUNNEL_ADDR") {
            config.tunnel_addr = v;
        }
    }
}
