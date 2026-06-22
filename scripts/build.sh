#!/usr/bin/env bash
# =============================================================================
# NetTool 构建脚本
#
# 功能：
#   1. 编译 Rust 项目（cargo build --release）
#   2. 构建 Web 前端（cd web-admin && npm install && npm run build）
#   3. 复制产物到 dist 目录，形成可部署的产物集合
#
# 用法：
#   ./scripts/build.sh                # 完整构建（服务端 + 前端）
#   ./scripts/build.sh --server-only  # 仅构建服务端
#   ./scripts/build.sh --web-only     # 仅构建前端
# =============================================================================

set -euo pipefail

# 脚本所在目录与项目根目录
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
DIST_DIR="$PROJECT_ROOT/dist"

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m'

info()  { echo -e "${GREEN}[INFO]${NC}  $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
error() { echo -e "${RED}[ERROR]${NC} $*" >&2; }
step()  { echo -e "${BLUE}[STEP]${NC}  $*"; }

# ---------------------------- 参数解析 ----------------------------
BUILD_SERVER=true
BUILD_WEB=true
while [[ $# -gt 0 ]]; do
    case "$1" in
        --server-only)
            BUILD_WEB=false
            shift
            ;;
        --web-only)
            BUILD_SERVER=false
            shift
            ;;
        --help|-h)
            echo "用法: $0 [--server-only|--web-only]"
            echo "  不带参数时执行完整构建。"
            exit 0
            ;;
        *)
            error "未知参数: $1"
            exit 1
            ;;
    esac
done

cd "$PROJECT_ROOT"

# ---------------------------- 构建服务端 ----------------------------
if [ "$BUILD_SERVER" = true ]; then
    step "构建 Rust 服务端 (cargo build --release)"

    if ! command -v cargo >/dev/null 2>&1; then
        error "未检测到 cargo，请先安装 Rust 工具链 (https://rustup.rs)。"
        exit 1
    fi

    info "编译 net-tool-server ..."
    cargo build --release -p net-tool-server

    if [ ! -f "$PROJECT_ROOT/target/release/net-tool-server" ]; then
        error "编译产物未找到: target/release/net-tool-server"
        exit 1
    fi

    info "Rust 服务端编译完成。"

    # 顺带编译客户端二进制（可选，失败不阻断）
    info "编译 net-tool 客户端 ..."
    if cargo build --release -p net-tool-client; then
        info "客户端编译完成。"
    else
        warn "客户端编译失败，跳过（不影响服务端部署）。"
    fi
fi

# ---------------------------- 构建前端 ----------------------------
if [ "$BUILD_WEB" = true ]; then
    step "构建 Web 管理后台 (web-admin)"

    WEB_DIR="$PROJECT_ROOT/web-admin"
    if [ ! -d "$WEB_DIR" ]; then
        warn "未找到 web-admin 目录，跳过前端构建。"
    elif [ ! -f "$WEB_DIR/package.json" ]; then
        warn "web-admin/package.json 不存在，跳过前端构建。"
    else
        if ! command -v npm >/dev/null 2>&1; then
            error "未检测到 npm，请先安装 Node.js (https://nodejs.org)。"
            exit 1
        fi

        info "安装前端依赖 (npm install) ..."
        (cd "$WEB_DIR" && npm install)

        info "构建前端 (npm run build) ..."
        (cd "$WEB_DIR" && npm run build)

        if [ ! -d "$WEB_DIR/dist" ]; then
            error "前端构建产物未找到: web-admin/dist"
            exit 1
        fi
        info "Web 前端构建完成。"
    fi
fi

# ---------------------------- 汇总产物到 dist ----------------------------
step "汇总构建产物到 $DIST_DIR"

mkdir -p "$DIST_DIR"

if [ "$BUILD_SERVER" = true ] && [ -f "$PROJECT_ROOT/target/release/net-tool-server" ]; then
    cp "$PROJECT_ROOT/target/release/net-tool-server" "$DIST_DIR/"
    info "已复制: net-tool-server"
fi

if [ -f "$PROJECT_ROOT/target/release/net-tool" ]; then
    cp "$PROJECT_ROOT/target/release/net-tool" "$DIST_DIR/"
    info "已复制: net-tool (客户端)"
fi

if [ "$BUILD_WEB" = true ] && [ -d "$PROJECT_ROOT/web-admin/dist" ]; then
    rm -rf "$DIST_DIR/web-admin"
    cp -r "$PROJECT_ROOT/web-admin/dist" "$DIST_DIR/web-admin"
    info "已复制: web-admin/"
fi

if [ -f "$PROJECT_ROOT/config.toml" ]; then
    cp "$PROJECT_ROOT/config.toml" "$DIST_DIR/"
    info "已复制: config.toml"
fi

# ---------------------------- 完成 ----------------------------
echo
info "构建完成！产物位于: $DIST_DIR"
echo
echo "产物清单:"
( cd "$DIST_DIR" && find . -maxdepth 2 -type f | sort | sed 's/^/  /' )
echo
echo "部署方式:"
echo "  1. 将 dist/ 目录上传到目标服务器"
echo "  2. 运行 ./net-tool-server 启动服务端"
echo "  或使用 Docker: docker compose up -d"
