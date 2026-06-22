//! 普通客户端模式
//!
//! 流程：
//! 1. 连接服务端并注册节点（角色 `Client`）
//! 2. 扫描本机网卡，展示候选网段
//! 3. 交互式选择要上报的网段，填写名称与备注
//! 4. 调用 `POST /api/v1/segments/report` 上报，展示服务端分配的映射网段
//! 5. 启动心跳循环
//! 6. 进入命令循环：list / add / status / config / diag / quit

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use net_tool_common::{NodeRole, RegisterRequest};
use tokio::sync::Mutex;

use crate::config;
use crate::connection::{self, ConnectionStatus, ServerConnection};
use crate::detect_os;
use crate::diagnostic;
use crate::display;
use crate::scanner;
use crate::tunnel_client::{TunnelClient, TunnelHandle, get_tunnel_addr};
use crate::{read_line, CommonArgs};

/// 客户端模式入口。
pub async fn run(common: CommonArgs) -> Result<()> {
    let cfg = config::resolve(&common, NodeRole::Client)?;
    // 持久化配置，便于下次启动复用。
    let _ = config::save_config(&cfg, None);

    display::print_banner("普通客户端模式");
    config::print_config(&cfg);

    // 可选：启动前诊断。
    if common.diagnostic {
        println!("\n运行启动诊断...");
        let items = diagnostic::run_quick(&cfg.server_addr).await;
        diagnostic::print_diag(&items);
    }

    let mut conn = ServerConnection::new(&cfg.server_addr)?;
    println!("\n正在连接服务端 {} ...", cfg.server_addr);

    let reg = conn
        .register_with_retry(
            RegisterRequest {
                name: cfg.node_name.clone(),
                role: NodeRole::Client,
                os_type: detect_os(),
                remark: cfg.remark.clone(),
            },
            5,
        )
        .await?;

    println!(
        "{} 注册成功，节点 ID: {}",
        display::green("√"),
        reg.node_id
    );

    // 首次上报引导。
    let _ = add_segment_flow(&conn).await;

    // 共享状态。
    let status = Arc::new(Mutex::new(conn.status()));
    let running = Arc::new(AtomicBool::new(true));

    // 隧道客户端（延迟初始化）
    let tunnel_addr = get_tunnel_addr(&cfg.server_addr);
    let node_id = reg.node_id.clone();
    let tunnel: Arc<Mutex<Option<TunnelHandle>>> = Arc::new(Mutex::new(None));

    // 心跳任务。
    let hb_conn = conn.clone();
    let hb_status = status.clone();
    let hb_running = running.clone();
    tokio::spawn(async move {
        connection::run_heartbeat(hb_conn, hb_status, hb_running).await;
    });

    // 命令循环。
    let result = command_loop(conn, status, running, cfg, tunnel, tunnel_addr, node_id).await;
    result
}

/// 命令循环。
async fn command_loop(
    conn: ServerConnection,
    status: Arc<Mutex<ConnectionStatus>>,
    running: Arc<AtomicBool>,
    cfg: config::ClientConfig,
    tunnel: Arc<Mutex<Option<TunnelHandle>>>,
    tunnel_addr: String,
    node_id: String,
) -> Result<()> {
    print_help();
    loop {
        let line = read_line("\n[client] > ").await?;
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let cmd = parts.next().unwrap_or("");
        let rest: Vec<&str> = parts.collect();

        match cmd {
            "list" | "ls" => {
                match conn.query_segments().await {
                    Ok(segs) => display::print_reported_segments(&segs),
                    Err(e) => println!("{} 查询网段失败: {e}", display::red("×")),
                }
            }
            "add" => {
                if let Err(e) = add_segment_flow(&conn).await {
                    println!("{} 上报网段失败: {e}", display::red("×"));
                }
            }
            "status" | "st" => {
                print_status(&conn, &status, &cfg).await;
            }
            "config" | "cfg" => {
                config::print_config(&cfg);
            }
            "tunnel" => {
                let sub = rest.first().copied().unwrap_or("");
                match sub {
                    "start" => {
                        let mut guard = tunnel.lock().await;
                        if guard.is_some() {
                            println!("{} 隧道已在运行中", display::yellow("!"));
                        } else {
                            println!("正在启动隧道 {} ...", tunnel_addr);
                            let mut tc = TunnelClient::new(node_id.clone(), tunnel_addr.clone(), false);
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
                                            r.real_cidr,
                                            r.segment_name);
                                    }
                                    // 提取句柄，存入共享状态
                                    let handle = tc.handle();
                                    *guard = Some(handle);
                                    // 释放锁后再 spawn，避免死锁
                                    drop(guard);
                                    // 在后台运行隧道主循环
                                    let tunnel_clone = tunnel.clone();
                                    tokio::spawn(async move {
                                        if let Err(e) = tc.run().await {
                                            tracing::error!(error = %e, "隧道运行错误");
                                        }
                                        // 隧道结束后清理句柄
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
                    _ => {
                        println!("用法: tunnel <start|status|stop>");
                    }
                }
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
                // 给心跳任务和隧道清理一点时间
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                println!("{}", display::green("已退出。"));
                return Ok(());
            }
            other => {
                println!("{} 未知命令: {other}（输入 help 查看帮助）", display::yellow("!"));
            }
        }
        let _ = rest;
    }
}

/// 上报网段交互流程：扫描 -> 选择 -> 填写 -> 上报 -> 展示映射。
async fn add_segment_flow(conn: &ServerConnection) -> Result<()> {
    println!("\n扫描本机网卡...");
    let interfaces = scanner::scan_interfaces()?;
    if interfaces.is_empty() {
        println!("{}", display::yellow("未扫描到可用网卡。"));
        return Ok(());
    }
    display::print_interfaces(&interfaces);

    let sel = read_line("\n请输入要上报的网卡序号（多个用逗号分隔，直接回车跳过）: ").await?;
    if sel.trim().is_empty() {
        println!("已跳过上报。");
        return Ok(());
    }

    for token in sel.split(|c: char| c == ',' || c.is_whitespace()) {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        let idx: usize = token
            .parse()
            .map_err(|_| anyhow!("无效的序号: {token}"))?;
        if idx == 0 || idx > interfaces.len() {
            println!("{} 序号 {idx} 超出范围，已跳过。", display::yellow("!"));
            continue;
        }
        let iface = &interfaces[idx - 1];

        println!(
            "\n已选择 {} ({})，推测网段: {}",
            iface.name,
            iface.iface_type.as_str(),
            iface.cidr
        );
        let name = read_line("请输入网段名称: ").await?;
        if name.is_empty() {
            println!("{} 名称为空，已跳过该网卡。", display::yellow("!"));
            continue;
        }
        let remark = read_line("请输入备注（可留空）: ").await?;
        let remark = if remark.is_empty() {
            None
        } else {
            Some(remark)
        };

        let resp = conn
            .report_segment(&name, &iface.cidr, remark.clone())
            .await?;

        println!("{} 上报成功！", display::green("√"));
        println!("  网段名称 : {name}");
        println!("  真实网段 : {}", iface.cidr);
        println!("  映射网段 : {}", display::green(&resp.mapped_cidr));
        println!("  网段 ID  : {}", resp.segment_id);
        display::print_segment_mapping(&iface.cidr, &resp.mapped_cidr);
    }
    Ok(())
}

/// 打印状态信息。
async fn print_status(
    conn: &ServerConnection,
    status: &Arc<Mutex<ConnectionStatus>>,
    cfg: &config::ClientConfig,
) {
    let st = status.lock().await.clone();
    println!("---- 状态 ----");
    println!("连接状态 : {}", display::connection_status_colored(&st));
    println!("服务端   : {}", cfg.server_addr);
    println!("节点 ID  : {}", conn.node_id().unwrap_or("-"));
    println!("节点名称 : {}", cfg.node_name);
    match conn.query_segments().await {
        Ok(segs) => {
            println!("已上报网段数: {}", segs.len());
            let active = segs
                .iter()
                .filter(|s| matches!(s.status, net_tool_common::SegmentStatus::Active))
                .count();
            println!("其中已激活 : {active}");
        }
        Err(e) => println!("查询网段失败: {e}"),
    }
    println!("--------------");
}

fn print_help() {
    println!(
        "\n{} 可用命令：",
        display::bold("客户端命令")
    );
    println!("  {}  查看已上报网段", display::cyan("list"));
    println!("  {}   添加 / 上报新网段", display::cyan("add"));
    println!("  {}  查看连接与网段状态", display::cyan("status"));
    println!("  {}  查看当前配置", display::cyan("config"));
    println!("  {}  启动/查看/停止隧道", display::cyan("tunnel"));
    println!("  {}  运行诊断并导出", display::cyan("diag"));
    println!("  {}  显示帮助", display::cyan("help"));
    println!("  {}  退出并清理", display::cyan("quit"));
}
