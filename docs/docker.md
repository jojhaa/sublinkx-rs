# Docker 部署 / Docker Deployment

项目只保留一个 `docker-compose.yml`，默认直接使用 Docker Hub 镜像：

```text
docker.io/jojhaa/sublinkx-rs-backend:latest
docker.io/jojhaa/sublinkx-rs-frontend:latest
```

## 快速启动

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
admin / admin123456
```

首次登录后必须修改用户名和密码。密码会使用 Argon2 哈希后保存到 SQLite。

## 配置文件

`.env.example` 内容：

```env
FRONTEND_PORT=3000
SUBLINKX_DOCKER_SUBNET=172.31.88.0/24
BACKEND_IMAGE=docker.io/jojhaa/sublinkx-rs-backend:latest
FRONTEND_IMAGE=docker.io/jojhaa/sublinkx-rs-frontend:latest
BACKEND_DATA_DIR=./docker-data/backend
MIHOMO_CORE_DIR=./docker-data/mihomo
JWT_SECRET=change-me-in-production
JWT_EXP_HOURS=24
BOOTSTRAP_ADMIN_USERNAME=admin
BOOTSTRAP_ADMIN_PASSWORD=admin123456
```

生产部署前至少修改：

```env
JWT_SECRET=一串随机长密钥
```

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

如果使用 Nginx Proxy Manager、1Panel、宝塔或 Caddy，反代：

```text
http://127.0.0.1:3000
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

## English Quick Start

Only one compose file is needed:

```bash
cp .env.example .env
docker compose up -d
```

Images:

```text
docker.io/jojhaa/sublinkx-rs-backend:latest
docker.io/jojhaa/sublinkx-rs-frontend:latest
```

Open:

```text
http://localhost:3000
```

Default first login:

```text
admin / admin123456
```
