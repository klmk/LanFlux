//! NAT 配置
//!
//! 在客户端侧配置 NAT，将映射网段的流量转发到真实网段。
//!
//! ## Linux 实现
//!
//! 使用 `iptables` 配置 DNAT 和 MASQUERADE：
//! - DNAT：将目标为映射网段的包改写为目标为真实网段
//! - MASQUERADE：将源为真实网段的包进行源地址转换
//!
//! ## Windows 实现
//!
//! Windows NAT 配置较为复杂，初版仅记录日志，不实际配置。
//!
//! ## IP 转发
//!
//! [`enable_ip_forwarding`](NatManager::enable_ip_forwarding) 在 Linux 上
//! 通过写入 `/proc/sys/net/ipv4/ip_forward` 启用内核 IP 转发。

use anyhow::{Context, Result};

/// 单条 NAT 规则。
///
/// 描述从映射网段到真实网段的 NAT 映射关系。
#[derive(Debug, Clone)]
pub struct NatRule {
    /// 映射网段 CIDR，如 `"100.64.1.0/24"`。
    pub mapped_cidr: String,
    /// 真实网段 CIDR，如 `"192.168.1.0/24"`。
    pub real_cidr: String,
    /// 物理网卡名称。
    pub interface: String,
}

/// NAT 管理器。
///
/// 管理所有 NAT 规则，支持添加和批量清理。
pub struct NatManager {
    /// 已配置的 NAT 规则列表。
    pub rules: Vec<NatRule>,
}

impl Default for NatManager {
    fn default() -> Self {
        Self::new()
    }
}

impl NatManager {
    /// 创建一个空的 NAT 管理器。
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// 添加 NAT 规则。
    ///
    /// # 参数
    /// - `mapped_cidr`：映射网段，如 `"100.64.1.0/24"`
    /// - `real_cidr`：真实网段，如 `"192.168.1.0/24"`
    /// - `interface`：物理网卡名称
    ///
    /// # Linux 命令
    /// ```text
    /// iptables -t nat -A PREROUTING -d {mapped_cidr} -j DNAT --to-destination {real_cidr_base}
    /// iptables -t nat -A POSTROUTING -s {real_cidr} -o {interface} -j MASQUERADE
    /// ```
    ///
    /// `real_cidr_base` 为 `real_cidr` 去掉前缀后的网络地址部分。
    pub fn add_rule(&mut self, mapped_cidr: &str, real_cidr: &str, interface: &str) -> Result<()> {
        tracing::info!(
            mapped_cidr = mapped_cidr,
            real_cidr = real_cidr,
            interface = interface,
            "添加 NAT 规则"
        );

        add_nat_rule_impl(mapped_cidr, real_cidr, interface).with_context(|| {
            format!("添加 NAT 规则失败 (mapped={mapped_cidr}, real={real_cidr}, iface={interface})")
        })?;

        self.rules.push(NatRule {
            mapped_cidr: mapped_cidr.to_string(),
            real_cidr: real_cidr.to_string(),
            interface: interface.to_string(),
        });
        Ok(())
    }

    /// 清理所有 NAT 规则。
    ///
    /// 逆序删除所有通过 [`add_rule`](Self::add_rule) 添加的 NAT 规则。
    /// 单条规则删除失败不会中断整体清理流程。
    pub fn cleanup(&mut self) -> Result<()> {
        tracing::info!(count = self.rules.len(), "开始清理 NAT 规则");

        let rules = std::mem::take(&mut self.rules);
        let mut errors = Vec::new();

        for rule in rules.into_iter().rev() {
            if let Err(e) = delete_nat_rule_impl(&rule.mapped_cidr, &rule.real_cidr, &rule.interface) {
                tracing::warn!(
                    mapped_cidr = %rule.mapped_cidr,
                    real_cidr = %rule.real_cidr,
                    interface = %rule.interface,
                    error = %e,
                    "删除 NAT 规则失败（继续清理）"
                );
                errors.push(format!(
                    "mapped={}, real={}, iface={}: {}",
                    rule.mapped_cidr, rule.real_cidr, rule.interface, e
                ));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            anyhow::bail!("部分 NAT 规则清理失败: {}", errors.join("; "))
        }
    }

    /// 启用 IP 转发。
    ///
    /// # Linux
    /// 写入 `1` 到 `/proc/sys/net/ipv4/ip_forward`。
    ///
    /// # 其他平台
    /// 仅记录日志，不做实际操作。
    pub fn enable_ip_forwarding() -> Result<()> {
        enable_ip_forwarding_impl()
    }
}

// ============================================================
// 平台相关实现
// ============================================================

/// 从 CIDR 中提取网络地址部分（去掉 `/prefix`）。
fn cidr_base(cidr: &str) -> &str {
    cidr.split_once('/').map(|(base, _)| base).unwrap_or(cidr)
}

#[cfg(target_os = "linux")]
fn add_nat_rule_impl(mapped_cidr: &str, real_cidr: &str, interface: &str) -> Result<()> {
    let real_base = cidr_base(real_cidr);

    // DNAT: 将目标为映射网段的包改写为目标为真实网段
    let dnat_output = std::process::Command::new("iptables")
        .args([
            "-t", "nat", "-A", "PREROUTING",
            "-d", mapped_cidr,
            "-j", "DNAT",
            "--to-destination", real_base,
        ])
        .output()
        .context("执行 iptables DNAT 命令失败")?;

    if !dnat_output.status.success() {
        let stderr = String::from_utf8_lossy(&dnat_output.stderr);
        anyhow::bail!(
            "iptables DNAT 失败 (PREROUTING -d {mapped_cidr} -j DNAT --to-destination {real_base}): {stderr}"
        );
    }

    // MASQUERADE: 将源为真实网段的包进行源地址转换
    let masq_output = std::process::Command::new("iptables")
        .args([
            "-t", "nat", "-A", "POSTROUTING",
            "-s", real_cidr,
            "-o", interface,
            "-j", "MASQUERADE",
        ])
        .output()
        .context("执行 iptables MASQUERADE 命令失败")?;

    if !masq_output.status.success() {
        let stderr = String::from_utf8_lossy(&masq_output.stderr);
        anyhow::bail!(
            "iptables MASQUERADE 失败 (POSTROUTING -s {real_cidr} -o {interface} -j MASQUERADE): {stderr}"
        );
    }

    tracing::debug!(
        mapped_cidr = mapped_cidr,
        real_cidr = real_cidr,
        interface = interface,
        "NAT 规则添加成功 (DNAT + MASQUERADE)"
    );
    Ok(())
}

#[cfg(target_os = "linux")]
fn delete_nat_rule_impl(mapped_cidr: &str, real_cidr: &str, interface: &str) -> Result<()> {
    let real_base = cidr_base(real_cidr);

    // 删除 MASQUERADE 规则（先删后加的逆序）
    let masq_output = std::process::Command::new("iptables")
        .args([
            "-t", "nat", "-D", "POSTROUTING",
            "-s", real_cidr,
            "-o", interface,
            "-j", "MASQUERADE",
        ])
        .output()
        .context("执行 iptables 删除 MASQUERADE 命令失败")?;

    if !masq_output.status.success() {
        let stderr = String::from_utf8_lossy(&masq_output.stderr);
        tracing::debug!(stderr = %stderr, "删除 MASQUERADE 规则返回非零状态（可能规则已不存在）");
    }

    // 删除 DNAT 规则
    let dnat_output = std::process::Command::new("iptables")
        .args([
            "-t", "nat", "-D", "PREROUTING",
            "-d", mapped_cidr,
            "-j", "DNAT",
            "--to-destination", real_base,
        ])
        .output()
        .context("执行 iptables 删除 DNAT 命令失败")?;

    if !dnat_output.status.success() {
        let stderr = String::from_utf8_lossy(&dnat_output.stderr);
        tracing::debug!(stderr = %stderr, "删除 DNAT 规则返回非零状态（可能规则已不存在）");
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn enable_ip_forwarding_impl() -> Result<()> {
    tracing::info!("启用 Linux IP 转发");

    std::fs::write("/proc/sys/net/ipv4/ip_forward", "1")
        .context("写入 /proc/sys/net/ipv4/ip_forward 失败（需要 root 权限）")?;

    tracing::info!("Linux IP 转发已启用");
    Ok(())
}

// Windows 实现：初版仅记录日志
#[cfg(target_os = "windows")]
fn add_nat_rule_impl(mapped_cidr: &str, real_cidr: &str, interface: &str) -> Result<()> {
    tracing::warn!(
        mapped_cidr = mapped_cidr,
        real_cidr = real_cidr,
        interface = interface,
        "Windows NAT 配置尚未实现，跳过（初版）"
    );
    // 初版跳过 Windows NAT 配置
    Ok(())
}

#[cfg(target_os = "windows")]
fn delete_nat_rule_impl(_mapped_cidr: &str, _real_cidr: &str, _interface: &str) -> Result<()> {
    tracing::warn!("Windows NAT 清理尚未实现，跳过（初版）");
    Ok(())
}

#[cfg(target_os = "windows")]
fn enable_ip_forwarding_impl() -> Result<()> {
    tracing::warn!("Windows IP 转发启用尚未实现，跳过（初版）");
    // Windows 可通过注册表启用：
    // reg add HKLM\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters /v IPEnableRouter /t REG_DWORD /d 1 /f
    Ok(())
}

// 非 Linux / Windows 平台的回退实现
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn add_nat_rule_impl(mapped_cidr: &str, real_cidr: &str, interface: &str) -> Result<()> {
    tracing::warn!(
        mapped_cidr = mapped_cidr,
        real_cidr = real_cidr,
        interface = interface,
        "当前平台不支持 NAT 配置，跳过"
    );
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn delete_nat_rule_impl(_mapped_cidr: &str, _real_cidr: &str, _interface: &str) -> Result<()> {
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn enable_ip_forwarding_impl() -> Result<()> {
    tracing::warn!("当前平台不支持 IP 转发配置，跳过");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nat_manager_new() {
        let nm = NatManager::new();
        assert!(nm.rules.is_empty());
    }

    #[test]
    fn nat_manager_default() {
        let nm = NatManager::default();
        assert!(nm.rules.is_empty());
    }

    #[test]
    fn cleanup_empty() {
        let mut nm = NatManager::new();
        assert!(nm.cleanup().is_ok());
    }

    #[test]
    fn cidr_base_extraction() {
        assert_eq!(cidr_base("192.168.1.0/24"), "192.168.1.0");
        assert_eq!(cidr_base("10.0.0.1/8"), "10.0.0.1");
        assert_eq!(cidr_base("100.64.0.1"), "100.64.0.1"); // 无前缀
    }
}
