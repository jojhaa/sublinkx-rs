# Docker 部署

[English](docker.en.md)

本页说明如何使用 Docker Compose 部署 `sublinkx-rs`。

默认使用 Docker Hub 镜像：

```text
docker.io/jojhaa/sublinkx-rs-backend:latest
docker.io/jojhaa/sublinkx-rs-frontend:latest
```

## 快速启动

先从 GitHub 拉取部署配置到本地文件夹：

```bash
git clone https://github.com/jojhaa/sublinkx-rs.git
cd sublinkx-rs
```

这一步会获取 `docker-compose.yml`、`.env.example`、部署文档和默认目录结构。默认部署不会在本机编译源码。

```bash
cp .env.example .env
docker compose up -d
```

访问：

```text
http://服务器IP:3000
```

本机测试：

```text
http://localhost:3000
```

默认首次登录账号：

```text
使用你在 `.env` 中配置的 `BOOTSTRAP_ADMIN_USERNAME` / `BOOTSTRAP_ADMIN_PASSWORD`
```

首次登录后必须修改用户名和密码。密码会使用 Argon2 哈希后保存到 SQLite。

## 配置文件

复制 `.env.example` 后至少修改 `JWT_SECRET` 和首次引导管理员凭据：

```env
FRONTEND_PORT=3000
SUBLINKX_DOCKER_SUBNET=172.31.88.0/24
BACKEND_IMAGE=docker.io/jojhaa/sublinkx-rs-backend:latest
FRONTEND_IMAGE=docker.io/jojhaa/sublinkx-rs-frontend:latest
BACKEND_DATA_DIR=./docker-data/backend
MIHOMO_CORE_DIR=./docker-data/mihomo
DATABASE_URL=sqlite:///app/data/app.db
JWT_SECRET=请改成至少 32 位随机长密钥
JWT_EXP_HOURS=24
AUTH_COOKIE_SECURE=true
TRUST_PROXY_HEADERS=true
BOOTSTRAP_ADMIN_USERNAME=请改成自定义管理员用户名
BOOTSTRAP_ADMIN_PASSWORD=请改成强首次登录密码
```

生产部署建议：

```env
JWT_SECRET=请改成至少 32 位随机长密钥
AUTH_COOKIE_SECURE=true
```

`AUTH_COOKIE_SECURE=true` 是生产默认值，需要 HTTPS。仅本地纯 HTTP 开发时才设置为 `false`。
`TRUST_PROXY_HEADERS=true` 只能在后端仅能通过可信反向代理或前端容器代理访问时启用。

## 本地数据映射

默认把运行数据映射到本地文件夹：

```text
docker-data/
  backend/
    app.db
  mihomo/
    mihomo
```

映射关系：

```yaml
./docker-data/backend:/app/data
./docker-data/mihomo:/app/mihomo
```

也可以改成绝对路径。

Windows 示例：

```env
BACKEND_DATA_DIR=D:/sublinkx-data/backend
MIHOMO_CORE_DIR=D:/sublinkx-data/mihomo
```

Linux 示例：

```env
BACKEND_DATA_DIR=/opt/sublinkx-rs/data
MIHOMO_CORE_DIR=/opt/sublinkx-rs/mihomo
```

SQLite 数据库文件：

```text
BACKEND_DATA_DIR/app.db
```

Mihomo 内核目录：

```text
MIHOMO_CORE_DIR/
```

也可以登录后台后，在“系统设置”页面下载 Mihomo 内核。

## 使用 MySQL

默认使用 SQLite。如果你的 Linux 服务器 Docker bind mount 写入很慢，或者节点数量比较多，建议切换 MySQL。

### 方式一：使用 Compose 内置 MySQL 容器

这种方式会由当前 `docker-compose.yml` 一起启动 MySQL，并把数据映射到 `MYSQL_DATA_DIR`。

编辑 `.env`：

```env
COMPOSE_PROFILES=mysql
DATABASE_URL=mysql://sublinkx:<mysql_password>@mysql:3306/sublinkx
MYSQL_IMAGE=mysql:8.4
MYSQL_DATABASE=sublinkx
MYSQL_USER=sublinkx
MYSQL_PASSWORD=请改成强密码
MYSQL_ROOT_PASSWORD=请改成强密码
MYSQL_DATA_DIR=./docker-data/mysql
```

启动或重启：

```bash
docker compose up -d
```

MySQL 数据会映射到：

```text
docker-data/mysql/
```

### 方式二：连接本机或外部已有 MySQL

如果服务器上已经有 MySQL，不需要启用 `COMPOSE_PROFILES=mysql`。只需要把后端容器的 `DATABASE_URL` 指向已有 MySQL。

宿主机 MySQL 示例：

```env
DATABASE_URL=mysql://sublinkx:请改成强密码@host.docker.internal:3306/sublinkx
```

外部 MySQL 示例：

```env
DATABASE_URL=mysql://sublinkx:请改成强密码@192.168.1.10:3306/sublinkx
```

当前 Compose 已为 Linux Docker 添加 `host.docker.internal:host-gateway` 映射；Docker Desktop 上该域名默认可用。请确保 MySQL 用户允许来自 Docker 网段访问，并且目标数据库已创建。

注意：SQLite 和 MySQL 是两套独立数据库。切换前请先备份 `docker-data/backend/app.db`，当前版本不会自动迁移 SQLite 数据到 MySQL。

## 固定 Docker 网段

默认固定网段：

```env
SUBLINKX_DOCKER_SUBNET=172.31.88.0/24
```

如果和已有 Docker、VPN 或局域网网段冲突，改 `.env`：

```env
SUBLINKX_DOCKER_SUBNET=172.30.88.0/24
```

修改后重建网络：

```bash
docker compose down
docker compose up -d
```

## 端口和反向代理

默认只暴露前端：

```yaml
ports:
  - "3000:80"
```

后端只在 Docker 内部网络访问。前端 Nginx 会代理：

```text
/api/     -> backend:8080/api/
/s/       -> backend:8080/s/
/healthz  -> backend:8080/healthz
```

Compose 会对后端执行 `/healthz` 健康检查，前端只在后端健康后启动。如果前端日志出现 `backend could not be resolved`，优先检查后端是否启动失败：

```bash
docker compose ps
docker compose logs --tail=200 backend
```

如果使用 Nginx Proxy Manager、1Panel、宝塔或 Caddy，反代：

```text
http://127.0.0.1:3000
```

### 宝塔 / 外层 Nginx 推荐配置

宝塔仍然转发到前端 `3000`，让前端容器内部再转发到后端。不要把外层 Nginx 的 `/api/` 直接转发到 `127.0.0.1:8080`，因为默认 Compose 中后端只 `expose` 给 Docker 内部网络，并没有映射到宿主机端口。

运行时 Nginx CSP 使用 `connect-src 'self'`。生产部署应保持前端和 API 同源，并通过前端容器或等价的同源反代规则代理 `/api/`、`/s/`、`/healthz`。`script-src` 仅额外允许 `https://static.cloudflareinsights.com`，用于 Cloudflare 自动注入的 Web Analytics Beacon；自动注入模式通过本站同源 `/cdn-cgi/rum` 上报，因此不需要扩大 `connect-src`。如果没有启用 Cloudflare Web Analytics，可以在自定义 Nginx 配置中移除该脚本来源。

如果需要在宝塔里单独写 `/api/` 规则，也应该转发到 `3000`，并关闭缓存：

```nginx
location = /api/v1/nodes/test-latency-stream {
    proxy_pass http://127.0.0.1:3000;
    proxy_buffering off;
    proxy_cache off;
    proxy_no_cache 1;
    proxy_cache_bypass 1;

    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $remote_addr;
    proxy_set_header X-Forwarded-Proto $scheme;

    add_header Cache-Control "no-store" always;
}

location ^~ /api/ {
    proxy_pass http://127.0.0.1:3000;

    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Real-Port $remote_port;
    proxy_set_header X-Forwarded-For $remote_addr;
    proxy_set_header X-Forwarded-Proto $scheme;
    proxy_set_header X-Forwarded-Host $host;
    proxy_set_header X-Forwarded-Port $server_port;
    proxy_set_header REMOTE-HOST $remote_addr;

    proxy_connect_timeout 60s;
    proxy_send_timeout 600s;
    proxy_read_timeout 600s;

    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection $connection_upgrade;

    proxy_cache off;
    proxy_no_cache 1;
    proxy_cache_bypass 1;

    add_header Cache-Control "no-store, no-cache, must-revalidate, proxy-revalidate, max-age=0" always;
    add_header Pragma "no-cache" always;
    add_header Expires "0" always;
}
```

如果你在外层 Nginx / 宝塔里单独配置 `/s/` 订阅入口，建议关闭该 location 的访问日志，避免订阅 token 写入反代日志：

```nginx
location ^~ /s/ {
    access_log off;
    proxy_pass http://127.0.0.1:3000;

    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $remote_addr;
    proxy_set_header X-Forwarded-Proto $scheme;

    proxy_cache off;
    proxy_no_cache 1;
    proxy_cache_bypass 1;

    add_header Cache-Control "no-store, no-cache, must-revalidate, max-age=0" always;
    add_header Referrer-Policy "no-referrer" always;
}
```

前端页面也建议禁用 HTML 缓存，避免浏览器一直加载旧的 `index-*.js`：

```nginx
location ^~ / {
    proxy_pass http://127.0.0.1:3000;

    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $remote_addr;
    proxy_set_header X-Forwarded-Proto $scheme;

    proxy_cache off;
    proxy_no_cache 1;
    proxy_cache_bypass 1;

    add_header Cache-Control "no-store, no-cache, must-revalidate, max-age=0" always;
    add_header Pragma "no-cache" always;
    add_header Expires "0" always;
}
```

修改后执行：

```bash
nginx -t
systemctl reload nginx
```

如果宝塔开启过代理缓存，建议同时清理站点缓存目录，例如：

```bash
rm -rf /www/wwwroot/你的域名/proxy_cache_dir/*
```

## 常用命令

查看状态：

```bash
docker compose ps
```

查看日志：

```bash
docker compose logs -f
```

重启：

```bash
docker compose restart
```

停止：

```bash
docker compose down
```

升级镜像：

```bash
docker compose pull
docker compose up -d
```

备份 SQLite：

```bash
docker compose down
cp ./docker-data/backend/app.db ./app.db.bak
docker compose up -d
```

## 发布新镜像

登录 Docker Hub：

```bash
docker login docker.io -u jojhaa
```

Windows PowerShell：

```powershell
.\scripts\docker-push.ps1 -Namespace jojhaa -Tag latest
```

手动推送：

```powershell
docker tag sublinkx-rs-backend:local jojhaa/sublinkx-rs-backend:latest
docker push jojhaa/sublinkx-rs-backend:latest

docker tag sublinkx-rs-frontend:local jojhaa/sublinkx-rs-frontend:latest
docker push jojhaa/sublinkx-rs-frontend:latest
```

如果 Docker Hub 开启了 2FA，`docker login` 的密码需要填写 Docker Hub Access Token。
