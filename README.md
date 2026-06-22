# NetTool 组网工具

面向多局域网互通场景的组网软件。通过统一的中心服务端，让分布在不同网络环境中的电脑、服务器、虚拟机网段和现场设备网段建立可控连接，使实施人员能够稳定访问被授权的远端设备。

采用「一个本体、三种角色」的设计：同一个二进制支持 **服务端 (server)**、**客户端 (client)**、**实施端 (operator)** 三种运行模式。底层基于 TUN 虚拟网卡实现三层组网，支持 ICMP / TCP / UDP 转发。

## 核心特性

- 三层虚拟组网：基于 TUN 虚拟网卡，支持 ping (ICMP)、TCP、UDP
- 网段映射：客户端真实网段冲突时，由服务端统一分配平台映射网段
- 集中管理：服务端维护节点、网段、映射关系、权限与会话
- 单二进制多角色：`net-tool` 同时支持 server / client / operator 模式
- Web 管理后台：内置静态前端，开箱即用
- 轻量部署：支持 Docker 与原生 systemd 两种部署方式

## 目录结构

```
net-tool/
├── crates/                     # Rust workspace
│   ├── common/                 # 公共类型、协议定义
│   ├── network/                # 底层网络能力（TUN / 隧道 / 转发）
│   ├── server/                 # 服务端（HTTP API + Web 后台）
│   └── client/                 # 客户端 / 实施端 CLI
├── web-admin/                  # Web 管理后台前端源码
├── docker/                     # Docker 相关文件
│   └── entrypoint.sh           # 容器入口脚本
├── scripts/                    # 辅助脚本
│   ├── build.sh                # 构建脚本
│   └── install.sh              # Linux 安装脚本
├── Dockerfile                  # 多阶段构建镜像
├── docker-compose.yml          # Docker Compose 编排
├── config.toml                 # 服务端默认配置
├── .dockerignore
├── .gitignore
├── Cargo.toml                  # workspace 根 manifest
└── Cargo.lock
```

## 环境要求

- Rust 1.96+（编译服务端 / 客户端）
- Node.js 22+（构建 Web 前端）
- Docker 20.10+ 与 Docker Compose v2（容器化部署）
- Linux 内核支持 TUN（`/dev/net/tun`）

## 构建方法

### 一键构建

使用项目内置构建脚本，会依次编译 Rust 服务端/客户端、构建 Web 前端，并将产物汇总到 `dist/` 目录：

```bash
./scripts/build.sh
```

支持按需构建：

```bash
./scripts/build.sh --server-only   # 仅构建服务端
./scripts/build.sh --web-only      # 仅构建前端
```

### 手动构建

```bash
# 1. 编译 Rust 服务端与客户端
cargo build --release -p net-tool-server
cargo build --release -p net-tool-client

# 2. 构建 Web 前端
cd web-admin
npm install
npm run build
cd ..
```

构建产物：

| 产物 | 路径 | 说明 |
|---|---|---|
| 服务端二进制 | `target/release/net-tool-server` | 独立服务端 |
| 客户端二进制 | `target/release/net-tool` | client / operator CLI |
| Web 静态资源 | `web-admin/dist/` | 管理后台前端 |

## 部署方法

### 方式一：Docker 部署（推荐）

使用 Docker Compose 一键拉起：

```bash
# 构建并启动
docker compose up -d --build

# 查看日志
docker compose logs -f

# 停止
docker compose down
```

或使用 `docker run`：

```bash
docker build -t net-tool-server .

docker run -d \
  --name net-tool-server \
  --cap-add NET_ADMIN \
  --device /dev/net/tun \
  -p 8443:8443 \
  -v "$PWD/data:/data" \
  -v "$PWD/config.toml:/app/config.toml" \
  -e RUST_LOG=info \
  --restart unless-stopped \
  net-tool-server
```

> 容器运行需要 `NET_ADMIN` capability 与 `/dev/net/tun` 设备，已在 `docker-compose.yml` 中声明。这是 TUN 虚拟网卡工作所必需的。

启动后访问 Web 管理后台：`http://<服务器IP>:8443`

### 方式二：原生 systemd 部署

使用安装脚本自动完成编译、安装、注册 systemd 服务并启动：

```bash
# 本地编译安装
sudo ./scripts/install.sh

# 或下载预编译二进制安装
sudo ./scripts/install.sh --binary https://example.com/net-tool-server
```

安装脚本会：

1. 创建系统用户 `nettool` 与目录 `/opt/net-tool`、`/var/lib/net-tool`
2. 编译（或下载）二进制并复制到 `/opt/net-tool/`
3. 复制 Web 静态资源与配置文件
4. 创建 systemd 服务 `/etc/systemd/system/net-tool.service`
5. 启用开机自启并启动服务

服务管理：

```bash
systemctl status net-tool          # 查看状态
systemctl restart net-tool         # 重启
systemctl stop net-tool            # 停止
journalctl -u net-tool -f          # 查看日志
```

### 方式三：直接运行

```bash
# 准备配置与数据目录
mkdir -p data

# 运行服务端
./net-tool-server

# 或指定配置文件
NETTOOL_CONFIG=./config.toml ./net-tool-server
```

## 配置说明

服务端配置通过 `config.toml` 加载，优先级为：**环境变量 > 配置文件 > 代码默认值**。

配置文件查找顺序：

1. 环境变量 `NETTOOL_CONFIG` 指定的路径
2. 当前目录下的 `config.toml`
3. `/etc/nettool/config.toml`

### 配置项

| 配置项 | 环境变量 | 默认值 | 说明 |
|---|---|---|---|
| `listen_addr` | `NETTOOL_LISTEN_ADDR` | `0.0.0.0:8443` | HTTP 监听地址 |
| `database_url` | `NETTOOL_DATABASE_URL` | `sqlite://data/nettool.db?mode=rwc` | SQLite 连接串 |
| `web_admin_dir` | `NETTOOL_WEB_ADMIN_DIR` | `./web-admin` | Web 静态文件目录 |
| `log_level` | `NETTOOL_LOG_LEVEL` / `RUST_LOG` | `info` | 日志级别 |
| `client_pool_cidr` | `NETTOOL_CLIENT_POOL_CIDR` | `100.64.0.0/10` | 客户映射地址池 |
| `segment_size` | `NETTOOL_SEGMENT_SIZE` | `24` | 映射网段前缀长度 |
| `operator_pool_cidr` | `NETTOOL_OPERATOR_POOL_CIDR` | `100.127.0.0/24` | 实施端地址池 |
| `server_virtual_ip` | `NETTOOL_SERVER_VIRTUAL_IP` | `100.127.0.1` | 服务端虚拟 IP |

## 使用方法

### 服务端

服务端通常以 Docker 或 systemd 方式常驻运行，无需手动操作。Web 管理后台提供节点、网段、权限、地址池、会话、仪表盘等可视化管理。

### 客户端 / 实施端 CLI

客户端二进制 `net-tool` 支持三种子命令：

```bash
# 以服务端模式运行（轻量内嵌，生产环境建议使用独立服务端）
net-tool server --bind 0.0.0.0:8443

# 以普通客户端模式运行（上报真实网段）
net-tool client \
  --server-addr 127.0.0.1:8443 \
  --name "客户A-网关机" \
  --remark "现场网段 192.168.1.0/24"

# 以实施端模式运行（访问被授权的远端映射网段）
net-tool operator \
  --server-addr 127.0.0.1:8443 \
  --name "实施工程师-张三"

# 启动前先运行连通性诊断
net-tool client --server-addr 127.0.0.1:8443 --diagnostic

# 指定日志级别
net-tool operator --server-addr 127.0.0.1:8443 --log-level debug
```

公共参数：

| 参数 | 环境变量 | 说明 |
|---|---|---|
| `--server-addr` | `NET_TOOL_SERVER_ADDR` | 服务端地址 |
| `--name` | - | 节点名称 |
| `--remark` | - | 备注 |
| `--auto-reconnect` | - | 是否自动重连 |
| `--auto-start` | - | 是否开机自启 |
| `--config` | - | 配置文件路径（默认 `~/.net-tool/config.toml`） |
| `--diagnostic` | - | 启动前运行连通性诊断 |
| `--log-level` | - | 日志级别 |

## API 文档概要

服务端 HTTP API 统一前缀 `/api/v1`，响应体统一为 `ApiResponse<T>` 结构。同时 `/` 路径提供 Web 管理后台静态文件服务。

### 节点管理

| 方法 | 路径 | 说明 |
|---|---|---|
| POST | `/api/v1/nodes/register` | 节点注册（client / operator 上线时调用） |
| POST | `/api/v1/nodes/heartbeat` | 节点心跳上报 |
| GET | `/api/v1/nodes` | 节点列表 |
| GET | `/api/v1/nodes/:id` | 节点详情 |
| PUT | `/api/v1/nodes/:id` | 更新节点信息 |
| POST | `/api/v1/nodes/:id/disable` | 禁用节点 |
| POST | `/api/v1/nodes/:id/enable` | 启用节点 |
| POST | `/api/v1/nodes/:id/kick` | 踢下线 |

### 网段管理

| 方法 | 路径 | 说明 |
|---|---|---|
| POST | `/api/v1/segments/report` | 客户端上报真实网段 |
| GET | `/api/v1/segments` | 网段列表 |
| GET | `/api/v1/segments/:id` | 网段详情 |
| PUT | `/api/v1/segments/:id` | 更新网段 |
| POST | `/api/v1/segments/:id/enable` | 启用网段 |
| POST | `/api/v1/segments/:id/disable` | 禁用网段 |
| POST | `/api/v1/segments/:id/remap` | 重新分配映射网段 |
| GET | `/api/v1/mappings` | 映射关系列表 |

### 权限管理

| 方法 | 路径 | 说明 |
|---|---|---|
| GET | `/api/v1/policies` | 策略列表 |
| PUT | `/api/v1/policies/:node_id` | 更新节点策略 |
| GET | `/api/v1/policies/:node_id/detail` | 节点策略详情 |
| GET | `/api/v1/access` | 查询访问权限 |

### 地址池配置

| 方法 | 路径 | 说明 |
|---|---|---|
| GET | `/api/v1/pools` | 获取地址池配置 |
| PUT | `/api/v1/pools` | 更新地址池配置 |

### 连接会话

| 方法 | 路径 | 说明 |
|---|---|---|
| GET | `/api/v1/sessions` | 会话列表 |

### 仪表盘与日志

| 方法 | 路径 | 说明 |
|---|---|---|
| GET | `/api/v1/dashboard` | 仪表盘统计数据 |
| GET | `/api/v1/logs` | 系统日志 |

## 端口说明

| 端口 | 协议 | 用途 |
|---|---|---|
| 8443 | HTTP | 服务端 API + Web 管理后台 |

## 许可证

LGPL-3.0
