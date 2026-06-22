//! 配置管理
//!
//! 负责加载与保存客户端配置（基于 [`net_tool_common::ClientConfig`]）。
//!
//! 配置文件位置优先级：
//! 1. `--config` 指定的路径
//! 2. `~/.net-tool/config.toml`
//! 3. 当前目录下的 `.net-tool/config.toml`（当主目录不可写时回退）
//!
//! 命令行参数会覆盖配置文件中的同名项。

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use net_tool_common::NodeRole;

use crate::CommonArgs;

// 重新导出 ClientConfig，便于其它模块以 `config::ClientConfig` 引用。
pub use net_tool_common::ClientConfig;

/// 配置目录：优先 `~/.net-tool`，主目录不可用时回退到当前目录下的 `.net-tool`。
pub fn config_dir() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        PathBuf::from(home).join(".net-tool")
    } else {
        PathBuf::from(".net-tool")
    }
}

/// 默认配置文件路径。
pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

/// 确保配置目录存在。
pub fn ensure_config_dir() -> Result<()> {
    let dir = config_dir();
    if !dir.exists() {
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("创建配置目录失败: {}", dir.display()))?;
    }
    Ok(())
}

/// 从文件加载配置；文件不存在时返回默认配置。
pub fn load_config(path: Option<&Path>) -> Result<ClientConfig> {
    let p = match path {
        Some(p) => p.to_path_buf(),
        None => config_path(),
    };
    if p.exists() {
        let content = std::fs::read_to_string(&p)
            .with_context(|| format!("读取配置文件失败: {}", p.display()))?;
        let cfg: ClientConfig = toml::from_str(&content)
            .with_context(|| format!("解析配置文件失败: {}", p.display()))?;
        Ok(cfg)
    } else {
        Ok(ClientConfig::default())
    }
}

/// 保存配置到文件。
pub fn save_config(cfg: &ClientConfig, path: Option<&Path>) -> Result<()> {
    ensure_config_dir()?;
    let p = match path {
        Some(p) => p.to_path_buf(),
        None => config_path(),
    };
    let content = toml::to_string_pretty(cfg).context("序列化配置失败")?;
    std::fs::write(&p, content)
        .with_context(|| format!("写入配置文件失败: {}", p.display()))?;
    Ok(())
}

/// 根据命令行参数解析最终配置。
///
/// 流程：加载配置文件 -> 用子命令固定角色 -> 用 CLI 参数覆盖 -> 校验。
pub fn resolve(common: &CommonArgs, default_role: NodeRole) -> Result<ClientConfig> {
    let path = common.config.as_ref().map(PathBuf::from);
    let mut cfg = load_config(path.as_deref())?;

    // 角色由子命令决定，不可被配置文件覆盖。
    cfg.role = default_role;

    if let Some(addr) = &common.server_addr {
        cfg.server_addr = addr.clone();
    }
    if let Some(name) = &common.name {
        cfg.node_name = name.clone();
    }
    if let Some(remark) = &common.remark {
        cfg.remark = Some(remark.clone());
    }
    if let Some(ar) = common.auto_reconnect {
        cfg.auto_reconnect = ar;
    }
    if let Some(as_) = common.auto_start {
        cfg.auto_start = as_;
    }
    if let Some(level) = &common.log_level {
        cfg.log_level = level.clone();
    }

    if cfg.server_addr.trim().is_empty() {
        bail!("服务端地址不能为空，请通过 --server-addr 或配置文件指定");
    }
    if cfg.node_name.trim().is_empty() {
        // 名称为空时自动生成一个，避免注册被拒。
        cfg.node_name = format!("node-{}", &uuid::Uuid::new_v4().to_string()[..8]);
    }

    Ok(cfg)
}

/// 打印当前配置（隐藏敏感信息，目前无敏感字段）。
pub fn print_config(cfg: &ClientConfig) {
    println!("---- 当前配置 ----");
    println!("服务端地址 : {}", cfg.server_addr);
    println!("节点名称   : {}", cfg.node_name);
    println!("节点角色   : {}", role_name(&cfg.role));
    println!("备注       : {}", cfg.remark.clone().unwrap_or_default());
    println!("自动重连   : {}", if cfg.auto_reconnect { "是" } else { "否" });
    println!("开机自启   : {}", if cfg.auto_start { "是" } else { "否" });
    println!("日志级别   : {}", cfg.log_level);
    println!("配置文件   : {}", config_path().display());
    println!("------------------");
}

/// 角色中文名。
pub fn role_name(role: &NodeRole) -> &'static str {
    match role {
        NodeRole::Server => "服务端",
        NodeRole::Client => "客户端",
        NodeRole::Operator => "实施端",
    }
}
