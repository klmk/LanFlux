#!/usr/bin/env bash
# =============================================================================
# NetTool Linux 安装脚本
#
# 功能：
#   1. 下载或本地编译 net-tool-server 二进制
#   2. 创建安装目录 /opt/net-tool
#   3. 复制二进制、配置文件与 Web 静态资源
#   4. 创建 systemd 服务文件并启用开机自启
#   5. 启动服务
#
# 用法：
#   sudo ./scripts/install.sh                  # 本地编译安装
#   sudo ./scripts/install.sh --binary <url>   # 从指定 URL 下载预编译二进制
#
# 需以 root 权限运行。
# =============================================================================

set -euo pipefail

# ---------------------------- 配置区 ----------------------------
INSTALL_DIR="/opt/net-tool"
CONFIG_FILE="/etc/net-tool/config.toml"
DATA_DIR="/var/lib/net-tool"
SERVICE_FILE="/etc/systemd/system/net-tool.service"
SERVICE_NAME="net-tool"
USER_NAME="nettool"

# 脚本所在目录（用于定位项目源码）
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m'

info()  { echo -e "${GREEN}[INFO]${NC}  $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
error() { echo -e "${RED}[ERROR]${NC} $*" >&2; }

# ---------------------------- 前置检查 ----------------------------
if [ "$(id -u)" -ne 0 ]; then
    error "请以 root 权限运行此脚本（使用 sudo）。"
    exit 1
fi

if ! command -v systemctl >/dev/null 2>&1; then
    error "未检测到 systemctl，本脚本仅支持 systemd 发行版。"
    exit 1
fi

# ---------------------------- 参数解析 ----------------------------
BINARY_URL=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --binary)
            BINARY_URL="$2"
            shift 2
            ;;
        --help|-h)
            echo "用法: sudo $0 [--binary <url>]"
            echo "  不带参数时使用本地源码编译安装。"
            exit 0
            ;;
        *)
            error "未知参数: $1"
            exit 1
            ;;
    esac
done

# ---------------------------- 创建用户 ----------------------------
if ! id "$USER_NAME" >/dev/null 2>&1; then
    info "创建系统用户: $USER_NAME"
    useradd --system --no-create-home --shell /usr/sbin/nologin "$USER_NAME"
fi

# ---------------------------- 创建目录 ----------------------------
info "创建安装目录: $INSTALL_DIR"
mkdir -p "$INSTALL_DIR"
mkdir -p "$(dirname "$CONFIG_FILE")"
mkdir -p "$DATA_DIR"

# ---------------------------- 获取二进制 ----------------------------
BIN_PATH="$INSTALL_DIR/net-tool-server"

if [ -n "$BINARY_URL" ]; then
    # 从远程下载预编译二进制
    info "从远程下载二进制: $BINARY_URL"
    if ! command -v curl >/dev/null 2>&1; then
        apt-get update -qq && apt-get install -y -qq curl
    fi
    curl -fSL "$BINARY_URL" -o "$BIN_PATH"
else
    # 本地编译
    info "本地编译 Rust 服务端..."
    if ! command -v cargo >/dev/null 2>&1; then
        error "未检测到 cargo，请先安装 Rust 工具链 (https://rustup.rs) 或使用 --binary 参数下载预编译版本。"
        exit 1
    fi
    (cd "$PROJECT_ROOT" && cargo build --release -p net-tool-server)
    cp "$PROJECT_ROOT/target/release/net-tool-server" "$BIN_PATH"
fi

chmod +x "$BIN_PATH"

# ---------------------------- 复制 Web 静态资源 ----------------------------
if [ -d "$PROJECT_ROOT/web-admin/dist" ]; then
    info "复制 Web 管理后台静态资源"
    rm -rf "$INSTALL_DIR/web-admin"
    cp -r "$PROJECT_ROOT/web-admin/dist" "$INSTALL_DIR/web-admin"
else
    warn "未找到 web-admin/dist，跳过 Web 静态资源复制（Web 后台将不可用）。"
fi

# ---------------------------- 复制配置文件 ----------------------------
if [ ! -f "$CONFIG_FILE" ]; then
    info "写入默认配置文件: $CONFIG_FILE"
    if [ -f "$PROJECT_ROOT/config.toml" ]; then
        cp "$PROJECT_ROOT/config.toml" "$CONFIG_FILE"
    else
        cat > "$CONFIG_FILE" <<'EOF'
listen_addr = "0.0.0.0:8443"
database_url = "sqlite:///var/lib/net-tool/nettool.db?mode=rwc"
web_admin_dir = "/opt/net-tool/web-admin"
log_level = "info"

[address_pool]
client_pool_cidr = "100.64.0.0/10"
segment_size = 24
operator_pool_cidr = "100.127.0.0/24"
server_virtual_ip = "100.127.0.1"
EOF
    fi
else
    info "配置文件已存在，保留现有配置: $CONFIG_FILE"
fi

# ---------------------------- 设置权限 ----------------------------
chown -R "$USER_NAME":"$USER_NAME" "$INSTALL_DIR" "$DATA_DIR"
chown -R root:root "$(dirname "$CONFIG_FILE")"

# ---------------------------- 创建 systemd 服务 ----------------------------
info "创建 systemd 服务文件: $SERVICE_FILE"
cat > "$SERVICE_FILE" <<EOF
[Unit]
Description=NetTool Server (组网工具服务端)
Documentation=https://github.com/net-tool/net-tool
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=$USER_NAME
Group=$USER_NAME
WorkingDirectory=$INSTALL_DIR
ExecStart=$INSTALL_DIR/net-tool-server
Restart=on-failure
RestartSec=5
LimitNOFILE=65536

# 环境变量
Environment="RUST_LOG=info"
Environment="NETTOOL_CONFIG=$CONFIG_FILE"
Environment="NETTOOL_LISTEN_ADDR=0.0.0.0:8443"
Environment="NETTOOL_DATABASE_URL=sqlite://$DATA_DIR/nettool.db?mode=rwc"
Environment="NETTOOL_WEB_ADMIN_DIR=$INSTALL_DIR/web-admin"

# 安全加固
NoNewPrivileges=true
ProtectSystem=full
ProtectHome=true
PrivateTmp=true

# TUN 网卡相关能力
AmbientCapabilities=CAP_NET_ADMIN
CapabilityBoundingSet=CAP_NET_ADMIN

[Install]
WantedBy=multi-user.target
EOF

# ---------------------------- 启用并启动服务 ----------------------------
info "重新加载 systemd 配置"
systemctl daemon-reload

info "启用开机自启"
systemctl enable "$SERVICE_NAME"

info "启动服务"
systemctl restart "$SERVICE_NAME"

# ---------------------------- 完成 ----------------------------
sleep 2
if systemctl is-active --quiet "$SERVICE_NAME"; then
    info "安装完成！服务已启动。"
    echo
    echo "  查看状态:   systemctl status $SERVICE_NAME"
    echo "  查看日志:   journalctl -u $SERVICE_NAME -f"
    echo "  停止服务:   systemctl stop $SERVICE_NAME"
    echo "  重启服务:   systemctl restart $SERVICE_NAME"
    echo "  配置文件:   $CONFIG_FILE"
    echo "  数据目录:   $DATA_DIR"
    echo "  Web 后台:   http://<服务器IP>:8443"
else
    error "服务启动失败，请查看日志: journalctl -u $SERVICE_NAME -n 50"
    exit 1
fi
