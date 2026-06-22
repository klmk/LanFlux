//! 配置结构
//!
//! 定义服务端与客户端的配置文件结构，支持通过 TOML / JSON 等格式加载。

use serde::{Deserialize, Serialize};

use crate::models::{AddressPool, NodeRole};

/// 服务端配置
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServerConfig {
    /// 监听地址，如 `0.0.0.0:8443`
    pub listen_addr: String,
    /// 数据库连接字符串，如 `sqlite:nettool.db`
    pub database_url: String,
    /// web-admin 静态文件目录
    pub web_admin_dir: String,
    /// 日志级别，如 `info`
    pub log_level: String,
    /// 地址池配置
    pub address_pool: AddressPool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            listen_addr: "0.0.0.0:8443".to_string(),
            database_url: "sqlite:nettool.db".to_string(),
            web_admin_dir: "web-admin".to_string(),
            log_level: "info".to_string(),
            address_pool: AddressPool::default(),
        }
    }
}

/// 客户端配置
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClientConfig {
    /// 服务端地址，格式为 `IP:port` 或 `domain:port`
    pub server_addr: String,
    /// 节点名称
    pub node_name: String,
    /// 节点角色
    pub role: NodeRole,
    /// 备注
    pub remark: Option<String>,
    /// 是否自动重连
    pub auto_reconnect: bool,
    /// 是否开机自启
    pub auto_start: bool,
    /// 日志级别，如 `info`
    pub log_level: String,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            server_addr: "127.0.0.1:8443".to_string(),
            node_name: "unnamed-node".to_string(),
            role: NodeRole::Client,
            remark: None,
            auto_reconnect: true,
            auto_start: false,
            log_level: "info".to_string(),
        }
    }
}
