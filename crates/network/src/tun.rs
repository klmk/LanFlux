//! TUN 虚拟网卡管理
//!
//! 使用 [`tun_rs`] crate 创建和管理 TUN 虚拟网卡。
//! TUN 设备工作在三层（IP 层），用于读取和写入 IP 数据包。
//!
//! ## 基本用法
//!
//! ```no_run
//! use net_tool_network::tun::TunInterface;
//!
//! // 创建 TUN 网卡（需要 root 权限）
//! let mut tun = TunInterface::create("nettool0", "100.64.1.1", 24).unwrap();
//! println!("已创建 TUN 网卡: {}", tun.name);
//!
//! // 获取异步设备用于读写 IP 包
//! let mut dev = tun.get_async_reader().unwrap();
//! ```

use anyhow::{Context, Result};

/// TUN 虚拟网卡管理器。
///
/// 封装 [`tun_rs::SyncDevice`]，提供创建、转换为异步设备、关闭等功能。
/// 创建时需要 root / 管理员权限。
pub struct TunInterface {
    /// 网卡名称，如 `"nettool0"`。
    pub name: String,
    /// 网卡 IP 地址，如 `"100.64.1.1"`。
    pub ip: String,
    /// 网卡 CIDR，如 `"100.64.1.1/24"`。
    pub cidr: String,
    /// 底层同步设备。在调用 [`get_async_reader`](Self::get_async_reader) 后被取出。
    pub device: Option<tun_rs::SyncDevice>,
}

impl TunInterface {
    /// 创建 TUN 虚拟网卡。
    ///
    /// # 参数
    /// - `name`：网卡名称（如 `"nettool0"`）
    /// - `ip`：IP 地址（如 `"100.64.1.1"`）
    /// - `prefix`：前缀长度（如 `24`）
    ///
    /// # 权限
    /// 需要 root / 管理员权限。
    pub fn create(name: &str, ip: &str, prefix: u8) -> Result<Self> {
        tracing::info!(name = name, ip = ip, prefix = prefix, "正在创建 TUN 虚拟网卡");

        let device = tun_rs::DeviceBuilder::new()
            .name(name)
            .ipv4(ip, prefix, None)
            .build_sync()
            .with_context(|| format!("创建 TUN 设备失败 (name={name}, ip={ip}/{prefix})"))?;

        tracing::info!(name = name, ip = ip, prefix = prefix, "TUN 虚拟网卡创建成功");

        Ok(Self {
            name: name.to_string(),
            ip: ip.to_string(),
            cidr: format!("{ip}/{prefix}"),
            device: Some(device),
        })
    }

    /// 获取异步读取器（用于异步读取和写入 IP 包）。
    ///
    /// 此方法会消耗内部的同步设备，转换为 [`tun_rs::AsyncDevice`]。
    /// 调用后 [`device`](Self::device) 字段将变为 `None`。
    pub fn get_async_reader(&mut self) -> Result<tun_rs::AsyncDevice> {
        let sync_dev = self
            .device
            .take()
            .context("TUN 设备不存在（可能已被取出或关闭）")?;

        let async_dev = tun_rs::AsyncDevice::new(sync_dev)
            .context("将同步 TUN 设备转换为异步设备失败")?;

        tracing::debug!(name = %self.name, "已获取异步 TUN 设备");
        Ok(async_dev)
    }

    /// 关闭并清理 TUN 网卡。
    ///
    /// 尝试禁用设备后释放资源。设备被 drop 时底层 fd 会自动关闭。
    pub fn close(&mut self) -> Result<()> {
        if let Some(device) = self.device.take() {
            tracing::info!(name = %self.name, "正在关闭 TUN 虚拟网卡");
            // 尝试禁用设备，失败不阻塞关闭流程
            if let Err(e) = device.enabled(false) {
                tracing::warn!(name = %self.name, error = %e, "禁用 TUN 设备时出错（忽略）");
            }
            // device 在此处被 drop，底层 fd 自动关闭
        }
        Ok(())
    }
}

impl Drop for TunInterface {
    fn drop(&mut self) {
        if self.device.is_some() {
            let _ = self.close();
        }
    }
}

/// 从 TUN 设备异步读取一个 IP 包。
///
/// 将数据读入 `buf`，返回实际读取的字节数。
/// 如果 `buf` 长度不足以容纳完整的数据包，多余部分会被丢弃。
pub async fn read_packet(device: &mut tun_rs::AsyncDevice, buf: &mut [u8]) -> Result<usize> {
    let n = device
        .recv(buf)
        .await
        .context("从 TUN 设备异步读取 IP 包失败")?;
    tracing::trace!(len = n, "从 TUN 设备读取到 IP 包");
    Ok(n)
}

/// 向 TUN 设备异步写入一个 IP 包。
///
/// 将 `packet` 完整写入 TUN 设备。
pub async fn write_packet(device: &mut tun_rs::AsyncDevice, packet: &[u8]) -> Result<()> {
    device
        .send(packet)
        .await
        .with_context(|| format!("向 TUN 设备异步写入 IP 包失败 ({} 字节)", packet.len()))?;
    tracing::trace!(len = packet.len(), "已向 TUN 设备写入 IP 包");
    Ok(())
}
