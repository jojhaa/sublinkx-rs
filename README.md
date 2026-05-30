# sublinkx-rs

[English](README.en.md)

`sublinkx-rs` 是一个面向多协议、多客户端订阅管理的 Rust + Vue 3 项目。本项目基于 [gooaclok819/sublinkX](https://github.com/gooaclok819/sublinkX) 进行二次修改、架构重构与功能扩展，保留订阅分发的核心思路，同时将后端重构为 Rust/Axum，前端重构为 Vue 3 管理控制台，并以 SQLite 作为默认数据存储。

## 项目来源

- 原项目：[gooaclok819/sublinkX](https://github.com/gooaclok819/sublinkX)
- 当前项目：基于原项目思路进行二次开发，重点增强多协议导入、客户端模板导出、上游模板透传、真实链路测速、订阅生命周期和 Docker 部署体验。
- 说明：本仓库不是原项目的官方仓库；如需了解原始实现，请访问上方原项目链接。

## 致谢

感谢 [gooaclok819/sublinkX](https://github.com/gooaclok819/sublinkX) 原项目提供的订阅分发思路和实现参考。`sublinkx-rs` 的设计起点来自原项目，当前仓库在此基础上进行了 Rust + Vue 3 方向的重构和扩展。请在使用、修改或二次分发本项目时，同时尊重原项目的开源贡献和许可要求。

## 与原项目相比新增了什么

| 方向 | 原项目 | sublinkx-rs 当前增强 |
| --- | --- | --- |
| 技术栈 | 原项目实现 | 后端重构为 Rust/Axum，前端重构为 Vue 3，默认使用 SQLite |
| 管理后台 | 以原订阅管理能力为核心 | 增加节点、订阅、模板、分组、系统设置、语言切换等完整管理界面 |
| 节点导入 | 基础订阅导入 | 支持手动多行导入、整段 Base64 订阅自动解码、上游订阅链接导入、Mihomo YAML 节点提取 |
| 上游模板 | 主要偏转换输出 | 增加上游 Mihomo 模板保存与透传，适合不希望二次转换破坏复杂规则的订阅 |
| 多客户端导出 | 原有客户端适配思路 | 扩展 Mihomo/Clash Meta、Clash、Xray、Surge、sing-box、Quantumult X、Loon、Surfboard、Mellow、ClashR、SS SIP002/SIP008、Trojan URI 等目标 |
| 模板能力 | 基础模板方向 | 增加客户端模板管理、Clash/Mihomo 分流模板、Surge/sing-box/Quantumult X 等渲染方向 |
| 转换保真 | 依赖导出结果人工确认 | 增加“上游字段 vs 二次导出字段”的保真检查，用于发现协议字段丢失 |
| 订阅生命周期 | 基础订阅分发 | 增加启用/停用、过期时间、快捷续期、分组、节点筛选、自动识别客户端链接 |
| 延迟测试 | 非核心能力 | 集成 Mihomo 内核做真实链路测速，保存延迟、测速时间和不可用状态 |
| 安全 | 基础账号配置 | 默认首次登录强制修改账号密码，密码使用 Argon2 哈希保存 |
| 部署 | 手动部署为主 | 增加 Docker Hub 镜像、单一 `docker-compose.yml`、本地数据映射、固定 Docker 网段 |
| 文档 | 原项目文档 | 增加中英文分离 README、Docker 部署文档、兼容矩阵、协议 x 客户端设计说明 |

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
- 数据库：默认使用 SQLite，也支持切换到 MySQL 8.x。

## Docker 部署

项目只保留一个 `docker-compose.yml`，默认使用已发布镜像：

```text
docker.io/jojhaa/sublinkx-rs-backend:latest
docker.io/jojhaa/sublinkx-rs-frontend:latest
```

### 1. 准备配置

先从 GitHub 拉取部署配置到本地文件夹：

```bash
git clone https://github.com/jojhaa/sublinkx-rs.git
cd sublinkx-rs
```

这一步会获取 `docker-compose.yml`、`.env.example` 和相关部署文档。Docker 启动时仍会默认拉取 Docker Hub 上已发布的镜像。

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

默认使用 SQLite，运行数据映射到本地：

```text
docker-data/
  backend/
    app.db
  mihomo/
    mihomo
```

如果 Linux 服务器上 SQLite bind mount 很慢，建议切换 MySQL：

使用 Compose 内置 MySQL 容器：

```env
COMPOSE_PROFILES=mysql
DATABASE_URL=mysql://sublinkx:sublinkx_password@mysql:3306/sublinkx
MYSQL_PASSWORD=请改成强密码
MYSQL_ROOT_PASSWORD=请改成强密码
```

使用宿主机或外部已有 MySQL：

```env
DATABASE_URL=mysql://sublinkx:请改成强密码@host.docker.internal:3306/sublinkx
```

更完整的 MySQL 容器、本机 MySQL、外部 MySQL 配置见 [Docker 部署文档](docs/docker.md)。

然后启动：

```bash
docker compose up -d
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

本项目使用 [MIT License](LICENSE)。

本项目基于 [gooaclok819/sublinkX](https://github.com/gooaclok819/sublinkX) 二次修改与重构，原项目同样采用 MIT License。感谢原作者的开源贡献；使用、修改或分发本项目时，请保留本项目和原项目的版权与许可证声明。
