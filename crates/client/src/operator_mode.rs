//! 实施端模式
//!
//! 流程：
//! 1. 连接服务端并注册节点（角色 `Operator`）
//! 2. 申请虚拟 IP 与可访问网段（`POST /api/v1/operators/request-ip`）
//! 3. 展示可访问网段列表（客户/节点、网段名称、真实网段、映射网段）
//! 4. **自动启动隧道**：连接服务端隧道端口，创建 TUN 网卡，添加路由
//! 5. 启动心跳循环
//! 6. 进入命令循环：list / ping / tcp / udp / convert / tunnel / status / quit
//!
//! 访问测试（通过隧道直接使用映射 IP）：
//! - ping：调用系统 `ping` 命令（ping 映射 IP，经 TUN 隧道转发）
//! - TCP：使用 [`tokio::net::TcpStream::connect`] + 超时
//! - UDP：使用 [`tokio::net::UdpSocket`]
//!
//! IP 换算：输入真实 IP 输出映射 IP，输入映射 IP 输出真实 IP。
//!
//! 隧道管理：`tunnel start|status|stop|reconnect`

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use net_tool_common::{NodeRole, RegisterRequest, RouteEntry};
use net_tool_network as net;
use tokio::sync::Mutex;

use crate::config;
use crate::connection::{self, ConnectionStatus, ServerConnection};
use crate::cli::detect_os;
use crate::diagnostic;
use crate::display;
use crate::tunnel_client::{TunnelClient, TunnelHandle, get_tunnel_addr};
use crate::cli::{read_line, CommonArgs};

/// 实施端模式入口。
pub async fn run(common: CommonArgs) -> Result<()> {
    let cfg = config::resolve(&common, NodeRole::Operator)?;
    let _ = config::save_config(&cfg, None);

    display::print_banner("实施端模式");
    config::print_config(&cfg);

    if common.diagnostic {
        println!("\n运行启动诊断...");
        let items = diagnostic::run_quick(&cfg.server_addr).await;
        diagnostic::print_diag(&items);
    }

    let mut conn = ServerConnection::new(&cfg.server_addr)?;
    println!("\n正在连接服务端 {} ...", cfg.server_addr);

    let _reg = conn
        .register_with_retry(
            RegisterRequest {
                name: cfg.node_name.clone(),
                role: NodeRole::Operator,
                os_type: detect_os(),
                remark: cfg.remark.clone(),
            },
            5,
        )
        .await?;

    // 申请虚拟 IP 与可访问网段。
    let ip_resp = conn.request_operator_ip().await?;
    println!(
        "{} 已分配虚拟 IP: {}",
        display::green("√"),
        display::green(&ip_resp.virtual_ip)
    );

    let mut segments = ip_resp.accessible_segments;
    display::print_accessible_segments(&segments);

    // 共享状态。
    let status = Arc::new(Mutex::new(conn.status()));
    let running = Arc::new(AtomicBool::new(true));

    // 隧道客户端（实施端自动启动）
    let tunnel_addr = get_tunnel_addr(&cfg.server_addr);
    let node_id = conn.node_id().unwrap_or_default().to_string();
    let tunnel: Arc<Mutex<Option<TunnelHandle>>> = Arc::new(Mutex::new(None));

    // 实施端自动启动隧道
    if !segments.is_empty() {
        println!("\n正在自动启动隧道 {} ...", tunnel_addr);
        let mut tc = TunnelClient::new(node_id.clone(), tunnel_addr.clone(), true);
        match tc.connect().await {
            Ok(ack) => {
                println!("{} 隧道认证成功", display::green("√"));
                if let Some(ref vip) = ack.virtual_ip {
                    println!("  虚拟 IP: {}", display::green(vip));
                }
                println!("  可访问路由: {} 条", ack.routes.len());
                for r in &ack.routes {
                    println!("    {} -> {} ({})",
                        display::green(&r.mapped_cidr),
                        r.real_cidr, r.segment_name);
                }
                let handle = tc.handle();
                *tunnel.lock().await = Some(handle);
                // 在后台运行隧道主循环
                let tunnel_clone = tunnel.clone();
                tokio::spawn(async move {
                    if let Err(e) = tc.run().await {
                        tracing::error!(error = %e, "隧道运行错误");
                    }
                    *tunnel_clone.lock().await = None;
                    println!("{} 隧道已断开", display::yellow("!"));
                });
                println!("{} 隧道已启动，可通过映射 IP 访问远程网段", display::green("√"));
            }
            Err(e) => {
                println!("{} 隧道自动启动失败: {e}", display::red("×"));
                println!("  可使用 tunnel start 手动启动");
            }
        }
    } else {
        println!("{} 暂无可访问网段，隧道未启动", display::yellow("!"));
    }

    let hb_conn = conn.clone();
    let hb_status = status.clone();
    let hb_running = running.clone();
    tokio::spawn(async move {
        connection::run_heartbeat(hb_conn, hb_status, hb_running).await;
    });

    command_loop(conn, status, running, cfg, &mut segments, tunnel, tunnel_addr, node_id).await
}

/// 命令循环。
async fn command_loop(
    conn: ServerConnection,
    status: Arc<Mutex<ConnectionStatus>>,
    running: Arc<AtomicBool>,
    cfg: config::ClientConfig,
    segments: &mut Vec<RouteEntry>,
    tunnel: Arc<Mutex<Option<TunnelHandle>>>,
    tunnel_addr: String,
    node_id: String,
) -> Result<()> {
    print_help();
    loop {
        let line = read_line("\n[operator] > ").await?;
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let cmd = parts.next().unwrap_or("");
        let args: Vec<&str> = parts.collect();

        match cmd {
            "list" | "ls" => {
                match conn.query_access().await {
                    Ok(resp) => {
                        *segments = resp.allowed_segments;
                        display::print_accessible_segments(segments);
                    }
                    Err(e) => println!("{} 查询可访问网段失败: {e}", display::red("×")),
                }
            }
            "ping" => {
                let ip = match args.first() {
                    Some(ip) => *ip,
                    None => {
                        println!("用法: ping <ip>");
                        continue;
                    }
                };
                if let Err(e) = ping_test(ip).await {
                    println!("{} {e}", display::red("×"));
                }
            }
            "tcp" => {
                let (ip, port) = match parse_ip_port(&args) {
                    Some(v) => v,
                    None => {
                        println!("用法: tcp <ip> <port>");
                        continue;
                    }
                };
                tcp_test(ip, port).await;
            }
            "udp" => {
                let (ip, port) = match parse_ip_port(&args) {
                    Some(v) => v,
                    None => {
                        println!("用法: udp <ip> <port>");
                        continue;
                    }
                };
                udp_test(ip, port).await;
            }
            "convert" | "conv" => {
                let ip = match args.first() {
                    Some(ip) => *ip,
                    None => {
                        println!("用法: convert <ip>");
                        continue;
                    }
                };
                match convert_ip(ip, segments) {
                    Ok((dir, result)) => {
                        println!("{} {ip} -> {} ({})", display::green("√"), result, dir);
                    }
                    Err(e) => println!("{} {e}", display::red("×")),
                }
            }
            "tunnel" => {
                let sub = args.first().copied().unwrap_or("status");
                match sub {
                    "start" => {
                        let mut guard = tunnel.lock().await;
                        if guard.is_some() {
                            println!("{} 隧道已在运行中", display::yellow("!"));
                        } else {
                            println!("正在启动隧道 {} ...", tunnel_addr);
                            let mut tc = TunnelClient::new(node_id.clone(), tunnel_addr.clone(), true);
                            match tc.connect().await {
                                Ok(ack) => {
                                    println!("{} 隧道认证成功", display::green("√"));
                                    if let Some(ref vip) = ack.virtual_ip {
                                        println!("  虚拟 IP: {}", display::green(vip));
                                    }
                                    println!("  可访问路由: {} 条", ack.routes.len());
                                    for r in &ack.routes {
                                        println!("    {} -> {} ({})",
                                            display::green(&r.mapped_cidr),
                                            r.real_cidr, r.segment_name);
                                    }
                                    let handle = tc.handle();
                                    *guard = Some(handle);
                                    drop(guard);
                                    let tunnel_clone = tunnel.clone();
                                    tokio::spawn(async move {
                                        if let Err(e) = tc.run().await {
                                            tracing::error!(error = %e, "隧道运行错误");
                                        }
                                        *tunnel_clone.lock().await = None;
                                        println!("{} 隧道已断开", display::yellow("!"));
                                    });
                                    println!("{} 隧道已启动", display::green("√"));
                                }
                                Err(e) => {
                                    println!("{} 隧道连接失败: {e}", display::red("×"));
                                }
                            }
                        }
                    }
                    "status" => {
                        let guard = tunnel.lock().await;
                        if let Some(ref h) = *guard {
                            let st = h.get_status().await;
                            println!("---- 隧道状态 ----");
                            println!("状态     : {}", st.as_str());
                            println!("隧道地址 : {}", tunnel_addr);
                            if let Some(ref vip) = h.virtual_ip {
                                println!("虚拟 IP  : {}", vip);
                            }
                            let routes = h.get_routes().await;
                            println!("路由数量 : {}", routes.len());
                            for r in &routes {
                                println!("  {} -> {} ({})",
                                    display::green(&r.mapped_cidr),
                                    r.real_cidr, r.segment_name);
                            }
                            println!("------------------");
                        } else {
                            println!("{} 隧道未启动，使用 tunnel start 启动", display::yellow("!"));
                        }
                    }
                    "stop" => {
                        let mut guard = tunnel.lock().await;
                        if let Some(ref h) = *guard {
                            h.stop().await;
                            *guard = None;
                            println!("{} 隧道停止中...", display::yellow("!"));
                        } else {
                            println!("{} 隧道未运行", display::yellow("!"));
                        }
                    }
                    "reconnect" => {
                        // 先停止
                        {
                            let mut guard = tunnel.lock().await;
                            if let Some(ref h) = *guard {
                                h.stop().await;
                                *guard = None;
                            }
                        }
                        // 等待清理完成
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        // 重新刷新可访问网段
                        match conn.query_access().await {
                            Ok(resp) => {
                                *segments = resp.allowed_segments;
                                display::print_accessible_segments(segments);
                            }
                            Err(e) => {
                                println!("{} 刷新可访问网段失败: {e}", display::red("×"));
                            }
                        }
                        // 重新启动
                        if segments.is_empty() {
                            println!("{} 暂无可访问网段，无法启动隧道", display::red("×"));
                            continue;
                        }
                        println!("正在重新启动隧道 {} ...", tunnel_addr);
                        let mut tc = TunnelClient::new(node_id.clone(), tunnel_addr.clone(), true);
                        match tc.connect().await {
                            Ok(ack) => {
                                println!("{} 隧道重连成功", display::green("√"));
                                if let Some(ref vip) = ack.virtual_ip {
                                    println!("  虚拟 IP: {}", display::green(vip));
                                }
                                let handle = tc.handle();
                                *tunnel.lock().await = Some(handle);
                                let tunnel_clone = tunnel.clone();
                                tokio::spawn(async move {
                                    if let Err(e) = tc.run().await {
                                        tracing::error!(error = %e, "隧道运行错误");
                                    }
                                    *tunnel_clone.lock().await = None;
                                    println!("{} 隧道已断开", display::yellow("!"));
                                });
                                println!("{} 隧道已启动", display::green("√"));
                            }
                            Err(e) => {
                                println!("{} 隧道重连失败: {e}", display::red("×"));
                            }
                        }
                    }
                    _ => {
                        println!("用法: tunnel <start|status|stop|reconnect>");
                    }
                }
            }
            "status" | "st" => {
                print_status(&conn, &status, &cfg, segments, &tunnel).await;
            }
            "config" | "cfg" => {
                config::print_config(&cfg);
            }
            "diag" => {
                let items = diagnostic::run_quick(&cfg.server_addr).await;
                diagnostic::print_diag(&items);
                match diagnostic::export_diagnostic(&cfg, &conn.status(), Some(&conn)).await {
                    Ok(p) => println!("诊断信息已导出: {}", p.display()),
                    Err(e) => println!("{} 导出诊断失败: {e}", display::red("×")),
                }
            }
            "help" | "?" => print_help(),
            "quit" | "exit" | "q" => {
                println!("正在退出并清理...");
                // 停止隧道
                {
                    let mut guard = tunnel.lock().await;
                    if let Some(ref h) = *guard {
                        h.stop().await;
                        *guard = None;
                    }
                }
                running.store(false, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(500)).await;
                println!("{}", display::green("已退出。"));
                return Ok(());
            }
            other => {
                println!("{} 未知命令: {other}（输入 help 查看帮助）", display::yellow("!"));
            }
        }
    }
}

/// 解析 `<ip> <port>` 参数。
fn parse_ip_port<'a>(args: &'a [&'a str]) -> Option<(&'a str, u16)> {
    let ip = args.first().copied()?;
    let port: u16 = args.get(1)?.parse().ok()?;
    Some((ip, port))
}

/// ping 测试（调用系统 ping 命令）。
async fn ping_test(ip: &str) -> Result<()> {
    let mut cmd = tokio::process::Command::new("ping");
    #[cfg(target_os = "windows")]
    {
        cmd.args(["-n", "4", "-w", "2000", ip]);
    }
    #[cfg(not(target_os = "windows"))]
    {
        cmd.args(["-c", "4", "-W", "2", ip]);
    }
    let output = cmd.output().await.context("执行 ping 命令失败")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stdout.is_empty() {
        print!("{stdout}");
    }
    if !stderr.is_empty() {
        eprint!("{stderr}");
    }
    if !output.status.success() {
        println!("{} ping {ip} 未成功（目标不可达或被过滤）", display::red("×"));
    }
    Ok(())
}

/// TCP 端口测试。
async fn tcp_test(ip: &str, port: u16) {
    let addr = format!("{ip}:{port}");
    let start = Instant::now();
    let fut = tokio::net::TcpStream::connect(&addr);
    match tokio::time::timeout(Duration::from_secs(3), fut).await {
        Ok(Ok(_stream)) => {
            println!(
                "{} TCP {addr} 可达（{} ms）",
                display::green("√"),
                start.elapsed().as_millis()
            );
        }
        Ok(Err(e)) => {
            println!("{} TCP {addr} 连接失败: {e}", display::red("×"));
        }
        Err(_) => {
            println!("{} TCP {addr} 连接超时（3s）", display::red("×"));
        }
    }
}

/// UDP 端口测试。
async fn udp_test(ip: &str, port: u16) {
    let addr = format!("{ip}:{port}");
    let sock = match tokio::net::UdpSocket::bind("0.0.0.0:0").await {
        Ok(s) => s,
        Err(e) => {
            println!("{} 绑定 UDP 套接字失败: {e}", display::red("×"));
            return;
        }
    };
    let _ = sock.connect(&addr).await;
    if let Err(e) = sock.send(&[0u8; 1]).await {
        println!("{} UDP {addr} 发送失败: {e}", display::red("×"));
        return;
    }
    let mut buf = [0u8; 1];
    match tokio::time::timeout(Duration::from_secs(2), sock.recv(&mut buf)).await {
        Ok(Ok(_n)) => {
            println!("{} UDP {addr} 有响应", display::green("√"));
        }
        Ok(Err(e)) => {
            println!("{} UDP {addr} 接收错误: {e}", display::yellow("!"));
        }
        Err(_) => {
            println!(
                "{} UDP {addr} 无响应（端口可能开放但无回包，或被过滤）",
                display::yellow("!")
            );
        }
    }
}

/// IP 换算：自动判断输入是真实 IP 还是映射 IP，输出对应换算结果。
fn convert_ip(ip: &str, segments: &[RouteEntry]) -> Result<(&'static str, String)> {
    for seg in segments {
        // 输入属于真实网段 -> 输出映射 IP
        if net::ip_in_cidr(ip, &seg.real_cidr)? {
            let mapped = net::convert_real_to_mapped(ip, &seg.real_cidr, &seg.mapped_cidr)?;
            return Ok(("真实 -> 映射", mapped));
        }
        // 输入属于映射网段 -> 输出真实 IP
        if net::ip_in_cidr(ip, &seg.mapped_cidr)? {
            let real = net::convert_mapped_to_real(ip, &seg.real_cidr, &seg.mapped_cidr)?;
            return Ok(("映射 -> 真实", real));
        }
    }
    Err(anyhow!("IP {ip} 不属于任何已知可访问网段"))
}

/// 打印状态。
async fn print_status(
    conn: &ServerConnection,
    status: &Arc<Mutex<ConnectionStatus>>,
    cfg: &config::ClientConfig,
    segments: &[RouteEntry],
    tunnel: &Arc<Mutex<Option<TunnelHandle>>>,
) {
    let st = status.lock().await.clone();
    println!("---- 状态 ----");
    println!("连接状态 : {}", display::connection_status_colored(&st));
    println!("服务端   : {}", cfg.server_addr);
    println!("节点 ID  : {}", conn.node_id().unwrap_or("-"));
    println!(
        "虚拟 IP  : {}",
        conn.virtual_ip().map(display::green).unwrap_or_else(|| "-".into())
    );
    println!("可访问网段数: {}", segments.len());
    // 隧道状态
    let guard = tunnel.lock().await;
    if let Some(ref h) = *guard {
        let ts = h.get_status().await;
        println!("隧道状态 : {}", ts.as_str());
        let routes = h.get_routes().await;
        println!("隧道路由 : {} 条", routes.len());
    } else {
        println!("隧道状态 : 未启动");
    }
    println!("--------------");
}

fn print_help() {
    println!("\n{} 可用命令：", display::bold("实施端命令"));
    println!("  {}            查看可访问网段", display::cyan("list"));
    println!("  {} <ip>         ping 测试（可直接 ping 映射 IP）", display::cyan("ping"));
    println!("  {} <ip> <port>  TCP 端口测试（可使用映射 IP）", display::cyan("tcp"));
    println!("  {} <ip> <port>  UDP 端口测试（可使用映射 IP）", display::cyan("udp"));
    println!("  {} <ip>     真实/映射 IP 换算", display::cyan("convert"));
    println!("  {} <start|status|stop|reconnect>  隧道管理", display::cyan("tunnel"));
    println!("  {}          查看状态（含隧道）", display::cyan("status"));
    println!("  {}          查看当前配置", display::cyan("config"));
    println!("  {}          运行诊断并导出", display::cyan("diag"));
    println!("  {}          显示帮助", display::cyan("help"));
    println!("  {}          退出并清理", display::cyan("quit"));
}
