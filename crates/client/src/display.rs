//! 命令行显示工具
//!
//! 提供不依赖外部 crate 的表格输出、ANSI 着色与网段映射关系展示。
//! 表格在计算列宽时会剔除 ANSI 转义序列，保证带颜色的单元格依然对齐。

use net_tool_common::{SegmentStatus, SegmentSummary};

use crate::connection::ConnectionStatus;
use crate::scanner::{InterfaceType, NetworkInterface};

/// ANSI 颜色与样式码。
#[allow(dead_code)]
pub mod color {
    pub const RESET: &str = "\x1b[0m";
    pub const BOLD: &str = "\x1b[1m";
    pub const DIM: &str = "\x1b[2m";
    pub const RED: &str = "\x1b[31m";
    pub const GREEN: &str = "\x1b[32m";
    pub const YELLOW: &str = "\x1b[33m";
    pub const BLUE: &str = "\x1b[34m";
    pub const MAGENTA: &str = "\x1b[35m";
    pub const CYAN: &str = "\x1b[36m";
    pub const GRAY: &str = "\x1b[90m";
}

/// 判断当前终端是否 likely 支持颜色。
fn color_enabled() -> bool {
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }
    std::env::var_os("TERM").map(|t| t != "dumb").unwrap_or(true)
}

/// 用指定颜色包裹文本（不支持颜色时原样返回）。
pub fn paint(color: &str, text: &str) -> String {
    if color_enabled() {
        format!("{color}{text}{}", color::RESET)
    } else {
        text.to_string()
    }
}

pub fn green(text: &str) -> String {
    paint(color::GREEN, text)
}
pub fn red(text: &str) -> String {
    paint(color::RED, text)
}
pub fn yellow(text: &str) -> String {
    paint(color::YELLOW, text)
}
pub fn cyan(text: &str) -> String {
    paint(color::CYAN, text)
}
pub fn gray(text: &str) -> String {
    paint(color::GRAY, text)
}
pub fn bold(text: &str) -> String {
    paint(color::BOLD, text)
}

/// 计算字符串的显示宽度（剔除 ANSI 转义序列后的字符数）。
fn display_width(s: &str) -> usize {
    let mut count = 0usize;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // 跳过形如 \x1b[...m 的转义序列
            if chars.peek() == Some(&'[') {
                chars.next();
                while let Some(p) = chars.next() {
                    if p.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            count += 1;
        }
    }
    count
}

/// 简单的文本表格。
pub struct Table {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl Table {
    /// 创建表格，传入表头。
    pub fn new(headers: &[&str]) -> Self {
        Self {
            headers: headers.iter().map(|s| s.to_string()).collect(),
            rows: Vec::new(),
        }
    }

    /// 追加一行。
    pub fn add_row(&mut self, row: &[String]) {
        let mut r = row.to_vec();
        // 行长度不足时补空，避免越界。
        while r.len() < self.headers.len() {
            r.push(String::new());
        }
        self.rows.push(r);
    }

    /// 渲染为字符串。
    pub fn render(&self) -> String {
        let mut widths: Vec<usize> = self
            .headers
            .iter()
            .map(|h| display_width(h))
            .collect();
        for row in &self.rows {
            for (i, cell) in row.iter().enumerate() {
                if i < widths.len() {
                    widths[i] = widths[i].max(display_width(cell));
                }
            }
        }

        let mut out = String::new();
        out.push_str(&render_row(&self.headers, &widths));
        out.push('\n');
        out.push_str(&render_separator(&widths));
        out.push('\n');
        for row in &self.rows {
            out.push_str(&render_row(row, &widths));
            out.push('\n');
        }
        out
    }

    /// 直接打印表格。
    pub fn print(&self) {
        print!("{}", self.render());
    }
}

fn render_separator(widths: &[usize]) -> String {
    let mut out = String::from("+");
    for &w in widths {
        out.push('-');
        for _ in 0..w {
            out.push('-');
        }
        out.push_str("-+");
    }
    out
}

fn render_row(cells: &[String], widths: &[usize]) -> String {
    let mut out = String::from("|");
    for (i, cell) in cells.iter().enumerate() {
        let w = widths.get(i).copied().unwrap_or(0);
        let pad = w.saturating_sub(display_width(cell));
        out.push(' ');
        out.push_str(cell);
        for _ in 0..pad {
            out.push(' ');
        }
        out.push_str(" |");
    }
    out
}

/// 连接状态着色。
pub fn connection_status_colored(status: &ConnectionStatus) -> String {
    let text = status.as_str();
    match status {
        ConnectionStatus::Connected => green(text),
        ConnectionStatus::Connecting | ConnectionStatus::Reconnecting => yellow(text),
        ConnectionStatus::Disconnected | ConnectionStatus::Failed => red(text),
    }
}

/// 网段状态着色。
pub fn segment_status_colored(status: &SegmentStatus) -> String {
    let (text, colored) = match status {
        SegmentStatus::Active => ("已激活", green("已激活")),
        SegmentStatus::Pending => ("待激活", yellow("待激活")),
        SegmentStatus::Disabled => ("已禁用", gray("已禁用")),
        SegmentStatus::Error => ("错误", red("错误")),
    };
    let _ = text;
    colored
}

/// 网卡类型着色。
pub fn interface_type_colored(ty: &InterfaceType) -> String {
    match ty {
        InterfaceType::Physical => cyan(ty.as_str()),
        InterfaceType::Wifi => green(ty.as_str()),
        InterfaceType::Loopback => gray(ty.as_str()),
        _ => ty.as_str().to_string(),
    }
}

/// 打印本机网卡扫描结果。
pub fn print_interfaces(interfaces: &[NetworkInterface]) {
    if interfaces.is_empty() {
        println!("{}", yellow("未扫描到可用网卡。"));
        return;
    }
    let mut table = Table::new(&[
        "#", "网卡名称", "类型", "IP 地址", "推测网段", "网关", "推荐",
    ]);
    for (i, iface) in interfaces.iter().enumerate() {
        table.add_row(&[
            (i + 1).to_string(),
            iface.name.clone(),
            interface_type_colored(&iface.iface_type),
            iface.ip_address.clone(),
            iface.cidr.clone(),
            iface.gateway.clone().unwrap_or_else(|| "-".into()),
            if iface.recommended {
                green("是")
            } else {
                gray("可选")
            },
        ]);
    }
    table.print();
    println!(
        "{} {}",
        gray("提示:"),
        gray("标记为「是」的物理网卡 / Wi-Fi 通常是需要上报的真实网段。")
    );
}

/// 打印客户端已上报网段列表。
pub fn print_reported_segments(segments: &[SegmentSummary]) {
    if segments.is_empty() {
        println!("{}", yellow("尚未上报任何网段，输入 add 添加。"));
        return;
    }
    let mut table = Table::new(&[
        "#", "网段名称", "真实网段", "映射网段", "状态",
    ]);
    for (i, s) in segments.iter().enumerate() {
        table.add_row(&[
            (i + 1).to_string(),
            s.name.clone(),
            s.real_cidr.clone(),
            s.mapped_cidr.clone().unwrap_or_else(|| "未分配".into()),
            segment_status_colored(&s.status),
        ]);
    }
    table.print();
}

/// 打印实施端可访问网段列表。
pub fn print_accessible_segments(
    segments: &[net_tool_common::RouteEntry],
) {
    if segments.is_empty() {
        println!("{}", yellow("当前没有可访问的网段。"));
        return;
    }
    let mut table = Table::new(&[
        "#", "客户/节点", "网段名称", "真实网段", "映射网段",
    ]);
    for (i, s) in segments.iter().enumerate() {
        table.add_row(&[
            (i + 1).to_string(),
            s.target_node_name.clone(),
            s.segment_name.clone(),
            s.real_cidr.clone(),
            s.mapped_cidr.clone(),
        ]);
    }
    table.print();
}

/// 展示单条网段的真实 <-> 映射映射关系。
pub fn print_segment_mapping(real_cidr: &str, mapped_cidr: &str) {
    println!(
        "  {} {} {}",
        cyan(real_cidr),
        gray("<->"),
        green(mapped_cidr)
    );
}

/// 打印横幅标题。
pub fn print_banner(title: &str) {
    let line = "=".repeat(title.chars().count() + 8);
    println!(
        "\n{} {} {}\n",
        bold(&line),
        bold(title),
        bold(&line)
    );
}
