#!/bin/sh
# =============================================================================
# NetTool 服务端 Docker 入口脚本
#
# 职责：
#   1. 确保数据目录 /data 存在且可写
#   2. 打印启动信息（版本、监听地址、数据目录等）
#   3. 执行 net-tool-server
# =============================================================================

set -e

# 数据目录（SQLite 数据库与运行时数据存放于此）
DATA_DIR="${NETTOOL_DATA_DIR:-/data}"

# 创建数据目录
if [ ! -d "$DATA_DIR" ]; then
    echo "[entrypoint] 创建数据目录: $DATA_DIR"
    mkdir -p "$DATA_DIR"
fi

if [ ! -w "$DATA_DIR" ]; then
    echo "[entrypoint] 警告: 数据目录 $DATA_DIR 不可写，服务可能无法持久化数据" >&2
fi

# 打印启动信息
cat <<EOF

============================================================
  NetTool Server
============================================================
  监听地址    : ${NETTOOL_LISTEN_ADDR:-0.0.0.0:8443}
  数据目录    : $DATA_DIR
  数据库      : ${NETTOOL_DATABASE_URL:-sqlite:///data/nettool.db?mode=rwc}
  Web 目录    : ${NETTOOL_WEB_ADMIN_DIR:-/app/web-admin}
  日志级别    : ${RUST_LOG:-info}
  配置文件    : ${NETTOOL_CONFIG:-/app/config.toml}
============================================================

EOF

# 检查 TUN 设备（仅警告，不阻断启动）
if [ ! -c /dev/net/tun ]; then
    echo "[entrypoint] 警告: /dev/net/tun 设备不存在。" >&2
    echo "[entrypoint]          三层组网功能将不可用，请以 --device /dev/net/tun 启动容器。" >&2
fi

echo "[entrypoint] 启动 net-tool-server ..."
echo "[entrypoint] 执行命令: $@"

# 执行传入的命令（默认为 ./net-tool-server）
exec "$@"
