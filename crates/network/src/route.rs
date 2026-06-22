//! 跨平台路由管理
//!
//! 使用系统命令添加和删除路由，将目标 CIDR 指向指定网卡。
//!
//! ## 平台支持
//!
//! - **Linux**：使用 `ip route` 命令
//! - **Windows**：使用 `route` 命令
//!
//! [`RouteManager`] 会记录所有已添加的路由，在 [`cleanup`](RouteManager::cleanup) 时自动删除。

use anyhow::{Context, Result};

/// 路由管理器。
///
/// 跟踪通过 [`add_route`](Self::add_route) 添加的所有路由，
/// 以便在 [`cleanup`](Self::cleanup) 时统一删除。
pub struct RouteManager {
    /// 已添加的路由 CIDR 列表。
    pub added_routes: Vec<String>,
    /// 与 `added_routes` 一一对应的接口/网关名称（内部使用）。
    route_interfaces: Vec<String>,
}

impl Default for RouteManager {
    fn default() -> Self {
        Self::new()
    }
}

impl RouteManager {
    /// 创建一个空的路由管理器。
    pub fn new() -> Self {
        Self {
            added_routes: Vec::new(),
            route_interfaces: Vec::new(),
        }
    }

    /// 添加路由：将目标 CIDR 指向指定网卡。
    ///
    /// # 参数
    /// - `target_cidr`：如 `"100.64.1.0/24"`
    /// - `interface`：网卡名称或网关 IP
    ///
    /// # 平台命令
    /// - Linux: `ip route add {target_cidr} dev {interface}`
    /// - Windows: `route add {target_cidr} {interface}`
    pub fn add_route(&mut self, target_cidr: &str, interface: &str) -> Result<()> {
        tracing::info!(cidr = target_cidr, interface = interface, "添加路由");

        add_route_impl(target_cidr, interface)
            .with_context(|| format!("添加路由失败 (cidr={target_cidr}, interface={interface})"))?;

        self.added_routes.push(target_cidr.to_string());
        self.route_interfaces.push(interface.to_string());
        Ok(())
    }

    /// 删除路由。
    ///
    /// # 参数
    /// - `target_cidr`：如 `"100.64.1.0/24"`
    /// - `interface`：网卡名称或网关 IP
    ///
    /// # 平台命令
    /// - Linux: `ip route del {target_cidr} dev {interface}`
    /// - Windows: `route delete {target_cidr} {interface}`
    pub fn delete_route(&mut self, target_cidr: &str, interface: &str) -> Result<()> {
        tracing::info!(cidr = target_cidr, interface = interface, "删除路由");

        delete_route_impl(target_cidr, interface)
            .with_context(|| format!("删除路由失败 (cidr={target_cidr}, interface={interface})"))?;

        // 从已记录列表中移除
        let interface_str = interface.to_string();
        if let Some(pos) = self
            .added_routes
            .iter()
            .zip(self.route_interfaces.iter())
            .position(|(c, iface)| *c == target_cidr && *iface == interface_str)
        {
            self.added_routes.remove(pos);
            self.route_interfaces.remove(pos);
        }
        Ok(())
    }

    /// 清理所有已添加的路由。
    ///
    /// 逆序删除所有通过 [`add_route`](Self::add_route) 添加的路由。
    /// 单条路由删除失败不会中断整体清理流程。
    pub fn cleanup(&mut self) -> Result<()> {
        tracing::info!(count = self.added_routes.len(), "开始清理路由");

        let routes: Vec<(String, String)> = self
            .added_routes
            .iter()
            .cloned()
            .zip(self.route_interfaces.iter().cloned())
            .collect();

        self.added_routes.clear();
        self.route_interfaces.clear();

        let mut errors = Vec::new();
        for (cidr, interface) in routes.into_iter().rev() {
            if let Err(e) = delete_route_impl(&cidr, &interface) {
                tracing::warn!(cidr = %cidr, interface = %interface, error = %e, "删除路由失败（继续清理）");
                errors.push(format!("{cidr} via {interface}: {e}"));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            anyhow::bail!("部分路由清理失败: {}", errors.join("; "))
        }
    }
}

// ============================================================
// 平台相关实现
// ============================================================

#[cfg(target_os = "linux")]
fn add_route_impl(target_cidr: &str, interface: &str) -> Result<()> {
    let output = std::process::Command::new("ip")
        .args(["route", "add", target_cidr, "dev", interface])
        .output()
        .context("执行 `ip route add` 命令失败")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("`ip route add {target_cidr} dev {interface}` 失败: {stderr}");
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn delete_route_impl(target_cidr: &str, interface: &str) -> Result<()> {
    let output = std::process::Command::new("ip")
        .args(["route", "del", target_cidr, "dev", interface])
        .output()
        .context("执行 `ip route del` 命令失败")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // 路由不存在时不算错误
        if stderr.contains("No such process") || stderr.contains("cannot find device") {
            tracing::debug!(cidr = target_cidr, "路由不存在，跳过删除");
            return Ok(());
        }
        anyhow::bail!("`ip route del {target_cidr} dev {interface}` 失败: {stderr}");
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn add_route_impl(target_cidr: &str, interface: &str) -> Result<()> {
    let output = std::process::Command::new("route")
        .args(["add", target_cidr, interface])
        .output()
        .context("执行 `route add` 命令失败")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("`route add {target_cidr} {interface}` 失败: {stderr}");
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn delete_route_impl(target_cidr: &str, interface: &str) -> Result<()> {
    let output = std::process::Command::new("route")
        .args(["delete", target_cidr, interface])
        .output()
        .context("执行 `route delete` 命令失败")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::warn!(cidr = target_cidr, stderr = %stderr, "删除路由时返回非零状态（可能路由已不存在）");
    }
    Ok(())
}

// 非 Linux / Windows 平台的回退实现
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn add_route_impl(target_cidr: &str, interface: &str) -> Result<()> {
    anyhow::bail!(
        "当前平台不支持路由管理 (cidr={target_cidr}, interface={interface})；仅支持 Linux 和 Windows"
    )
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn delete_route_impl(target_cidr: &str, interface: &str) -> Result<()> {
    anyhow::bail!(
        "当前平台不支持路由管理 (cidr={target_cidr}, interface={interface})；仅支持 Linux 和 Windows"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_manager_new() {
        let rm = RouteManager::new();
        assert!(rm.added_routes.is_empty());
        assert!(rm.route_interfaces.is_empty());
    }

    #[test]
    fn route_manager_default() {
        let rm = RouteManager::default();
        assert!(rm.added_routes.is_empty());
    }

    #[test]
    fn cleanup_empty() {
        let mut rm = RouteManager::new();
        assert!(rm.cleanup().is_ok());
    }
}
