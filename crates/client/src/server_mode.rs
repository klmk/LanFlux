//! 服务端模式（轻量）
//!
//! 提示用户：生产环境建议使用 Linux / Docker 部署独立服务端。
//!
//! 本模式会尝试启动独立的服务端二进制 `net-tool-server`（若存在于 PATH 或
//! 当前可执行文件同级目录），并接管其生命周期；若未找到，则打印部署指引、
//! 展示服务状态与 Web 后台地址。

use std::time::Duration;

use anyhow::Result;
use tokio::process::Command;

use crate::diagnostic;
use crate::display;
use crate::CommonArgs;

/// 服务端模式入口。
pub async fn run(bind: String, open: bool, common: CommonArgs) -> Result<()> {
    display::print_banner("服务端模式");

    print_guidance(&bind);

    // 尝试定位独立服务端二进制。
    let binary = find_server_binary();

    match binary {
        Some(path) => {
            println!("{} 找到独立服务端二进制: {}", display::green("√"), path);
            println!("正在启动...（按 Ctrl+C 停止）\n");
            if open {
                open_browser(&format!("http://{bind}"));
            }
            run_embedded(&path, &bind).await
        }
        None => {
            println!(
                "{} 未找到 net-tool-server 二进制。",
                display::yellow("!")
            );
            println!("请使用独立服务端二进制或 Docker 部署，例如：");
            println!("  {}  # 直接运行独立服务端", display::cyan("net-tool-server --bind 0.0.0.0:8443"));
            println!("  {}        # Docker 部署", display::cyan("docker run -p 8443:8443 net-tool-server"));
            println!("\nWeb 后台地址: {}", display::cyan(&format!("http://{bind}")));

            // 服务状态检查。
            check_status(&bind).await;

            // 可选诊断。
            if common.diagnostic {
                println!("\n运行诊断...");
                let items = diagnostic::run_quick(&bind).await;
                diagnostic::print_diag(&items);
            }
            Ok(())
        }
    }
}

/// 打印生产部署指引。
fn print_guidance(bind: &str) {
    println!("{} 生产环境部署建议：", display::bold("提示"));
    println!("  - 推荐在 Linux 服务器或 Docker 容器中部署独立服务端");
    println!("  - 服务端需要持久化数据库与公网可达地址");
    println!("  - 当前监听地址: {}", display::cyan(bind));
    println!();
}

/// 启动独立服务端二进制并等待其退出。
async fn run_embedded(binary: &str, bind: &str) -> Result<()> {
    let mut cmd = Command::new(binary);
    cmd.arg("--bind").arg(bind);
    // 继承标准输入输出，便于直接查看日志。
    let mut child = cmd.spawn().map_err(|e| {
        anyhow::anyhow!("启动服务端失败: {e}")
    })?;
    let pid = child
        .id()
        .map(|p| p.to_string())
        .unwrap_or_else(|| "-".into());
    println!("服务端 PID: {pid}");
    println!("Web 后台: {}", display::cyan(&format!("http://{bind}")));

    let status = child.wait().await?;
    println!("服务端已退出: {:?}", status);
    Ok(())
}

/// 检查服务端端口是否已有服务在监听。
async fn check_status(bind: &str) {
    let (host, port) = diagnostic::split_host_port(bind);
    println!("\n检查端口 {bind} 是否在监听...");
    let fut = tokio::net::TcpStream::connect((host.as_str(), port));
    match tokio::time::timeout(Duration::from_secs(2), fut).await {
        Ok(Ok(_)) => {
            println!(
                "{} 端口 {bind} 已有服务在监听，Web 后台可访问。",
                display::green("√")
            );
        }
        _ => {
            println!(
                "{} 端口 {bind} 暂无服务监听，请先启动服务端。",
                display::yellow("!")
            );
        }
    }
}

/// 查找 net-tool-server 二进制：当前可执行文件同级目录 -> PATH。
fn find_server_binary() -> Option<String> {
    let name = if cfg!(windows) {
        "net-tool-server.exe"
    } else {
        "net-tool-server"
    };

    // 1) 与当前可执行文件同级目录。
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join(name);
            if candidate.exists() {
                return Some(candidate.to_string_lossy().to_string());
            }
        }
    }

    // 2) 当前工作目录。
    let local = std::path::PathBuf::from(name);
    if local.exists() {
        return Some(local.to_string_lossy().to_string());
    }

    // 3) PATH 中查找（使用 which 风格的简单搜索）。
    if let Some(path) = which(name) {
        return Some(path);
    }
    None
}

/// 简易 which：遍历 PATH 环境变量查找可执行文件。
fn which(name: &str) -> Option<String> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().to_string());
        }
    }
    None
}

/// 跨平台打开浏览器。
fn open_browser(url: &str) {
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open")
            .arg(url)
            .spawn();
    }
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/c", "start", "", url])
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(url).spawn();
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        let _ = url;
    }
}
