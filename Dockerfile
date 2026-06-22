# =============================================================================
# NetTool 多阶段构建 Dockerfile
#
# 阶段一：构建 Rust 服务端二进制 (net-tool-server)
# 阶段二：构建 Web 管理后台静态资源 (web-admin)
# 阶段三：最小化运行时镜像
# =============================================================================

# -----------------------------------------------------------------------------
# 阶段一：构建 Rust 服务端
# -----------------------------------------------------------------------------
FROM rust:1.96-slim AS server-builder

# 安装编译所需的系统依赖（openssl / pkg-config 等）
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        pkg-config \
        libssl-dev \
        ca-certificates \
        make \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# 先复制 manifest 以利用 Docker 层缓存加速依赖编译
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# 编译 release 二进制（产物：target/release/net-tool-server）
RUN cargo build --release -p net-tool-server

# -----------------------------------------------------------------------------
# 阶段二：构建 Web 前端
# -----------------------------------------------------------------------------
FROM node:22-slim AS web-builder

WORKDIR /build

# 先复制 package 元数据以利用层缓存
COPY web-admin/package*.json ./web-admin/

RUN cd web-admin && npm install

# 复制前端源码并构建
COPY web-admin ./web-admin

RUN cd web-admin && npm run build

# -----------------------------------------------------------------------------
# 阶段三：最小化运行时镜像
# -----------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

# 安装运行时所需的最小依赖（libssl + ca-certificates + tini 作为 init）
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        libssl3 \
        ca-certificates \
        tini \
        curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# 复制服务端二进制
COPY --from=server-builder /build/target/release/net-tool-server /app/net-tool-server

# 复制 Web 管理后台静态文件
COPY --from=web-builder /build/web-admin/dist /app/web-admin

# 复制默认配置文件与入口脚本
COPY config.toml /app/config.toml
COPY docker/entrypoint.sh /app/entrypoint.sh
RUN chmod +x /app/entrypoint.sh /app/net-tool-server

# 创建数据目录（SQLite 数据库与运行时数据存放于此）
RUN mkdir -p /data

# 暴露服务端口
EXPOSE 8443

# 声明数据卷
VOLUME ["/data"]

# ----------------------------------------------------------------------
# 运行时权限与设备说明：
#
# 本服务基于 TUN 虚拟网卡实现三层组网，容器运行时需要：
#   1. NET_ADMIN capability —— 用于创建/配置 TUN 网卡与路由表
#   2. /dev/net/tun 设备    —— TUN 字符设备节点
#
# 使用 docker run 时请附加：
#   docker run --cap-add NET_ADMIN --device /dev/net/tun ...
#
# 在 docker-compose.yml 中已通过 cap_add / devices 声明。
# ----------------------------------------------------------------------

ENV RUST_LOG=info \
    NETTOOL_LISTEN_ADDR=0.0.0.0:8443 \
    NETTOOL_DATABASE_URL=sqlite:///data/nettool.db?mode=rwc \
    NETTOOL_WEB_ADMIN_DIR=/app/web-admin

ENTRYPOINT ["/usr/bin/tini", "--", "/app/entrypoint.sh"]
CMD ["./net-tool-server"]
