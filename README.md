# sublinkx-rs

[English](README.en.md)

`sublinkx-rs` 是一个使用 Rust + Vue 3 构建的多协议订阅管理控制台。

本项目基于 [gooaclok819/sublinkX](https://github.com/gooaclok819/sublinkX) 二次修改、重构和扩展。

## 项目来源与致谢

本项目基于 [gooaclok819/sublinkX](https://github.com/gooaclok819/sublinkX) 二次开发。感谢原项目提供的订阅分发思路和实现参考。

说明：

- 本仓库不是原项目官方仓库。
- 当前项目在原项目基础上进行了 Rust + Vue 3 方向的重构。
- 本项目和原项目均采用 MIT License，二次分发时请保留版权和许可证声明。

## 主要特性

- Rust 后端：基于 Axum、SQLx、Tokio，适合部署到 Linux 服务器。
- Vue 3 控制台：管理节点、节点分组、订阅、订阅分组、模板和系统设置。
- 多端 UI：桌面、平板、移动端均可用，节点和订阅支持详细/简约两种展示模式。
- 多协议导入：支持手动多行导入、整段 Base64 自动解码、上游订阅链接导入、Mihomo YAML 节点提取。
- 多客户端导出：支持 Mihomo/Clash Meta、Clash、Xray、Surge、sing-box、Quantumult X、Loon、Surfboard、Mellow、ClashR、SS SIP002/SIP008、Trojan URI 等目标。
- 上游模板透传：可以保存上游 Mihomo 模板，适合不希望复杂分流规则被二次转换破坏的订阅。
- 模板系统：支持客户端模板管理、Clash/Mihomo 分流模板，以及多个客户端方向的渲染器。
- 转换保真检查：对比上游 YAML proxy 字段和二次导出字段，帮助发现字段丢失。
- 真实链路测速：通过 Mihomo 内核测试真实代理链路延迟，不是 TCP ping。
- 延迟状态持久化：保存历史延迟、最后测速时间和不可用状态。
- 订阅生命周期：支持启用、停用、到期时间、快捷续期、节点筛选、分组和自动识别客户端链接。
- 首次登录安全：默认账号 `admin / admin123456`，首次登录后必须修改用户名和密码，密码使用 Argon2 哈希保存。
- Docker 部署：提供单一 `docker-compose.yml`，默认拉取 Docker Hub 镜像运行，数据映射到本地目录。
- 数据库：默认 SQLite，支持切换到 MySQL 8.x，可使用容器 MySQL、本机 MySQL 或外部 MySQL。

## 与原项目的主要区别

| 方向 | 原项目 | sublinkx-rs |
| --- | --- | --- |
| 技术栈 | 原项目实现 | 后端重构为 Rust/Axum，前端重构为 Vue 3 |
| 管理后台 | 以订阅分发为核心 | 增加节点、订阅、模板、分组、系统设置、语言切换和多端 UI |
| 节点导入 | 基础导入能力 | 增加 Base64 批量导入、上游订阅导入、Mihomo YAML 节点提取 |
| 上游模板 | 偏转换输出 | 支持保存上游 Mihomo 模板并透传导出 |
| 客户端导出 | 原有适配思路 | 扩展 Mihomo、Clash、Xray、Surge、sing-box、Quantumult X 等多客户端 |
| 分流模板 | 基础模板能力 | 增加 Clash/Mihomo 分流模板和多客户端模板管理 |
| 转换质量 | 依赖人工验证 | 增加字段保真检查和协议 x 客户端映射测试方向 |
| 延迟测试 | 非核心能力 | 集成 Mihomo 做真实链路测速，并保存结果 |
| 订阅管理 | 基础分发 | 增加启用/禁用、到期时间、分组、节点筛选和自动识别链接 |
| 部署 | 手动部署为主 | 增加 Docker Hub 镜像、Compose、本地数据映射、固定 Docker 网段 |

## Docker 快速部署

默认使用已发布镜像：

```text
docker.io/jojhaa/sublinkx-rs-backend:latest
docker.io/jojhaa/sublinkx-rs-frontend:latest
```

先拉取部署配置：

```bash
git clone https://github.com/jojhaa/sublinkx-rs.git
cd sublinkx-rs
cp .env.example .env
```

生产部署前至少修改 `.env` 中的密钥：

```env
JWT_SECRET=请改成一串足够长的随机密钥
```

启动：

```bash
docker compose up -d
```

访问：

```text
http://服务器IP:3000
```

默认首次登录：

```text
admin / admin123456
```

首次登录后系统会强制修改用户名和密码。

## 数据目录

Docker 默认把运行数据映射到本地：

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

## 数据库配置

默认使用 SQLite：

```env
DATABASE_URL=sqlite:///app/data/app.db
```

如果 Linux 服务器上 Docker bind mount 写入较慢，或者节点数量较多，建议使用 MySQL。

使用 Compose 内置 MySQL 容器：

```env
COMPOSE_PROFILES=mysql
DATABASE_URL=mysql://sublinkx:sublinkx_password@mysql:3306/sublinkx
MYSQL_IMAGE=mysql:8.4
MYSQL_DATABASE=sublinkx
MYSQL_USER=sublinkx
MYSQL_PASSWORD=请改成强密码
MYSQL_ROOT_PASSWORD=请改成强密码
MYSQL_DATA_DIR=./docker-data/mysql
```

使用宿主机或外部已有 MySQL：

```env
DATABASE_URL=mysql://sublinkx:请改成强密码@host.docker.internal:3306/sublinkx
```

Linux Docker 中 `host.docker.internal` 已通过 Compose 映射到宿主机网关。请确认 MySQL 用户允许 Docker 网段访问，并且数据库已创建。

更完整示例见：[Docker 部署文档](docs/docker.md)。

## 反向代理说明

默认 Compose 只把前端容器暴露到宿主机 `3000` 端口，后端 `8080` 只在 Docker 内部网络访问。

因此宝塔、1Panel、Nginx Proxy Manager、Caddy 或外层 Nginx 反代时，统一转发到：

```text
http://127.0.0.1:3000
```

不要直接转发到 `127.0.0.1:8080`，否则默认部署会出现 `502`。

前端容器内部 Nginx 会继续代理：

```text
/api/     -> backend:8080/api/
/s/       -> backend:8080/s/
/healthz  -> backend:8080/healthz
```

如果外层 Nginx 单独写 `/api/`，仍然应该转发到 `3000`，并禁用缓存：

```nginx
location ^~ /api/ {
    proxy_pass http://127.0.0.1:3000;

    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;

    proxy_cache off;
    proxy_no_cache 1;
    proxy_cache_bypass 1;

    add_header Cache-Control "no-store, no-cache, must-revalidate, proxy-revalidate, max-age=0" always;
    add_header Pragma "no-cache" always;
    add_header Expires "0" always;
}
```

## 固定 Docker 网段

默认网段：

```env
SUBLINKX_DOCKER_SUBNET=172.31.88.0/24
```

如果和现有 Docker、VPN 或局域网冲突，可以修改 `.env`：

```env
SUBLINKX_DOCKER_SUBNET=172.30.88.0/24
```

修改后重建网络：

```bash
docker compose down
docker compose up -d
```

## 常用 Docker 命令

```bash
docker compose ps
docker compose logs -f
docker compose restart
docker compose down
docker compose pull && docker compose up -d
```

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

真实链路测速依赖 Mihomo/Clash Meta 内核。

Docker 部署时默认目录：

```text
docker-data/mihomo/
```

本地开发时默认目录：

```text
backend/mihomo/
```

可以在后台“系统设置”页面检测并下载当前服务器系统对应的官方 MetaCubeX/mihomo 内核，也可以指定自定义内核路径。

## 文档

- [更新日志](CHANGELOG.md)
- [文档索引](docs/README.md)
- [Docker 部署](docs/docker.md)
- [重构蓝图](docs/plan.md)
- [客户端兼容矩阵](docs/client-compatibility.md)
- [协议 x 客户端矩阵](docs/protocol-client-matrix.md)
- [客户端目标注册表](docs/client-target-registry.md)
- [Clash 分流模板说明](docs/clash-routing-template.md)

## License

本项目使用 [MIT License](LICENSE)。

本项目基于 [gooaclok819/sublinkX](https://github.com/gooaclok819/sublinkX) 二次修改与重构。感谢原作者的开源贡献；使用、修改或分发本项目时，请保留本项目和原项目的版权与许可证声明。
