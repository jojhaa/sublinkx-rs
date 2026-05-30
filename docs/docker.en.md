# Docker Deployment

[中文](docker.md)

Only one `docker-compose.yml` is needed. It uses the published Docker Hub images by default:

```text
docker.io/jojhaa/sublinkx-rs-backend:latest
docker.io/jojhaa/sublinkx-rs-frontend:latest
```

Origin note: `sublinkx-rs` is a modified and rewritten version based on [gooaclok819/sublinkX](https://github.com/gooaclok819/sublinkX). The Docker images listed above are built from this repository.

## Quick Start

Clone the deployment configuration from GitHub first:

```bash
git clone https://github.com/jojhaa/sublinkx-rs.git
cd sublinkx-rs
```

This provides `docker-compose.yml`, `.env.example`, deployment docs, and the default local directory layout. The default deployment does not build from source locally; `docker-compose.yml` pulls the Docker Hub images:

```text
docker.io/jojhaa/sublinkx-rs-backend:latest
docker.io/jojhaa/sublinkx-rs-frontend:latest
```

```bash
cp .env.example .env
docker compose up -d
```

Open:

```text
http://server-ip:3000
```

Local test:

```text
http://localhost:3000
```

Default first login:

```text
admin / admin123456
```

The first login must change both username and password. Passwords are stored in SQLite as Argon2 hashes.

## Environment File

Copy `.env.example` and change at least `JWT_SECRET`:

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

Production recommendation:

```env
JWT_SECRET=use-a-long-random-secret
```

## Local Data Mapping

Runtime data is bind-mounted to local folders by default:

```text
docker-data/
  backend/
    app.db
  mihomo/
    mihomo
```

Mounts:

```yaml
./docker-data/backend:/app/data
./docker-data/mihomo:/app/mihomo
```

Absolute paths are also supported.

Windows example:

```env
BACKEND_DATA_DIR=D:/sublinkx-data/backend
MIHOMO_CORE_DIR=D:/sublinkx-data/mihomo
```

Linux example:

```env
BACKEND_DATA_DIR=/opt/sublinkx-rs/data
MIHOMO_CORE_DIR=/opt/sublinkx-rs/mihomo
```

SQLite database:

```text
BACKEND_DATA_DIR/app.db
```

Mihomo core directory:

```text
MIHOMO_CORE_DIR/
```

You can also download the Mihomo core from the Settings page after login.

## Fixed Docker Subnet

Default subnet:

```env
SUBLINKX_DOCKER_SUBNET=172.31.88.0/24
```

If it conflicts with existing Docker, VPN, or LAN networks, edit `.env`:

```env
SUBLINKX_DOCKER_SUBNET=172.30.88.0/24
```

Recreate the network after changing it:

```bash
docker compose down
docker compose up -d
```

## Ports and Reverse Proxy

Only the frontend is exposed by default:

```yaml
ports:
  - "3000:80"
```

The backend is only reachable inside the Docker network. The frontend Nginx container proxies:

```text
/api/     -> backend:8080/api/
/s/       -> backend:8080/s/
/healthz  -> backend:8080/healthz
```

For Nginx Proxy Manager, 1Panel, BT Panel, or Caddy, reverse proxy:

```text
http://127.0.0.1:3000
```

## Common Commands

Status:

```bash
docker compose ps
```

Logs:

```bash
docker compose logs -f
```

Restart:

```bash
docker compose restart
```

Stop:

```bash
docker compose down
```

Upgrade:

```bash
docker compose pull
docker compose up -d
```

Back up SQLite:

```bash
docker compose down
cp ./docker-data/backend/app.db ./app.db.bak
docker compose up -d
```

## Publish New Images

Login to Docker Hub:

```bash
docker login docker.io -u jojhaa
```

Windows PowerShell:

```powershell
.\scripts\docker-push.ps1 -Namespace jojhaa -Tag latest
```

Manual push:

```powershell
docker tag sublinkx-rs-backend:local jojhaa/sublinkx-rs-backend:latest
docker push jojhaa/sublinkx-rs-backend:latest

docker tag sublinkx-rs-frontend:local jojhaa/sublinkx-rs-frontend:latest
docker push jojhaa/sublinkx-rs-frontend:latest
```

If Docker Hub 2FA is enabled, use a Docker Hub access token as the `docker login` password.
