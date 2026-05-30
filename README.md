# sublinkx-rs

[English](README.en.md)

`sublinkx-rs` 是一个面向多协议、多客户端订阅管理的 Rust + Vue 3 项目。本项目基于 [gooaclok819/sublinkX](https://github.com/gooaclok819/sublinkX) 进行二次修改、架构重构与功能扩展，保留订阅分发的核心思路，同时将后端重构为 Rust/Axum，前端重构为 Vue 3 管理控制台，并以 SQLite 作为默认数据存储。

## 项目来源

- 原项目：[gooaclok819/sublinkX](https://github.com/gooaclok819/sublinkX)
- 当前项目：基于原项目思路进行二次开发，重点增强多协议导入、客户端模板导出、上游模板透传、真实链路测速、订阅生命周期和 Docker 部署体验。
- 说明：本仓库不是原项目的官方仓库；如需了解原始实现，请访问上方原项目链接。

## 项目特性

- Rust 后端：基于 Axum、SQLx 和 SQLite，适合轻量部署到 Linux 服务器。
- Vue 3 前端：提供节点、订阅、模板、分组、设置和导出管理界面。
- 多协议节点：支持 Shadowsocks、VMess、VLESS、Trojan、Hysteria2、TUIC、WireGuard、AnyTLS 等协议的解析和扩展。
- 多客户端导出：覆盖 Mihomo/Clash Meta、Clash、Xray、Surge、sing-box、Quantumult X、Loon、Surfboard、Mellow、ClashR、SS SIP002/SIP008、Trojan URI 等目标。
- 模板与保真检查：支持 Clash/Mihomo、Surge、sing-box、Quantumult X 等模板方向，并提供上游字段与二次导出字段的完整性检查。
- 上游订阅导入：支持从上游订阅链接导入节点，也支持保存上游 Mihomo 模板做透传导出，避免复杂配置被二次转换破坏。
- 订阅生命周期：支持启用、停用、到期时间、续期、分组、节点筛选和客户端自动识别链接。
- 真实链路测速：通过 Mihomo 内核测试真实代理链路延迟，并保存历史延迟、最后测速时间和不可用状态。
- 首次登录安全：默认账号 `admin / admin123456`，首次登录后必须修改用户名和密码，密码使用 Argon2 哈希保存。
- Docker 部署：提供单一 `docker-compose.yml`，默认拉取 Docker Hub 镜像运行，数据映射到本地文件夹。

## Docker 部署

项目只保留一个 `docker-compose.yml`，默认使用已发布镜像：

```text
docker.io/jojhaa/sublinkx-rs-backend:latest
docker.io/jojhaa/sublinkx-rs-frontend:latest
```

### 1. 准备配置

```bash
cp .env.example .env
```

生产部署前建议编辑 `.env`，至少修改：

```env
JWT_SECRET=请改成一串随机长密钥
```

### 2. 启动服务

```bash
docker compose up -d
```

### 3. 访问后台

```text
http://服务器IP:3000
```

本机测试：

```text
http://localhost:3000
```

默认首次登录：

```text
admin / admin123456
```

首次登录后必须修改用户名和密码。

### 4. 数据映射

运行数据默认映射到本地：

```text
docker-data/
  backend/
    app.db
  mihomo/
    mihomo
```

可以在 `.env` 中改成绝对路径：

```env
BACKEND_DATA_DIR=/opt/sublinkx-rs/data
MIHOMO_CORE_DIR=/opt/sublinkx-rs/mihomo
```

Windows 示例：

```env
BACKEND_DATA_DIR=D:/sublinkx-data/backend
MIHOMO_CORE_DIR=D:/sublinkx-data/mihomo
```

### 5. 固定 Docker 网段

默认固定网段：

```env
SUBLINKX_DOCKER_SUBNET=172.31.88.0/24
```

如果和现有 Docker、VPN 或局域网冲突，请修改 `.env`：

```env
SUBLINKX_DOCKER_SUBNET=172.30.88.0/24
```

修改后重建网络：

```bash
docker compose down
docker compose up -d
```

### 6. 常用命令

```bash
docker compose ps
docker compose logs -f
docker compose restart
docker compose down
docker compose pull && docker compose up -d
```

更多 Docker 部署说明见：[Docker 部署文档](docs/docker.md)。

## 本地开发

后端：

```powershell
cd backend
cargo run
```

前端：

```powershell
cd frontend
npm install
npm run dev -- --host 127.0.0.1 --port 5173
```

默认地址：

- 前端：http://127.0.0.1:5173
- 后端：http://127.0.0.1:8080

## Mihomo 内核

真实链路延迟测试依赖 Mihomo/Clash Meta 内核。Docker 部署时内核目录会映射到：

```text
docker-data/mihomo/
```

本地开发时默认查找：

```text
backend/mihomo/
```

也可以在后台“系统设置”页面一键检测并下载当前服务器系统对应的官方 MetaCubeX/mihomo 内核，或指定自定义内核路径。

## 文档

- [Docker 部署](docs/docker.md)
- [重构蓝图](docs/plan.md)
- [客户端兼容矩阵](docs/client-compatibility.md)
- [协议 x 客户端矩阵](docs/protocol-client-matrix.md)
- [客户端目标注册表](docs/client-target-registry.md)
- [Clash 分流模板说明](docs/clash-routing-template.md)

## License

License not specified yet. Please also review the license and attribution requirements of the upstream project [gooaclok819/sublinkX](https://github.com/gooaclok819/sublinkX) before redistribution.
