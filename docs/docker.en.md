# Docker Deployment

[中文](docker.md)

This page explains how to deploy `sublinkx-rs` with Docker Compose.

It uses the published Docker Hub images by default:

```text
docker.io/jojhaa/sublinkx-rs-backend:latest
docker.io/jojhaa/sublinkx-rs-frontend:latest
```

## Quick Start

Clone the deployment configuration from GitHub first:

```bash
git clone https://github.com/jojhaa/sublinkx-rs.git
cd sublinkx-rs
```

This provides `docker-compose.yml`, `.env.example`, deployment docs, and the default local directory layout. The default deployment does not build from source locally.

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
the `BOOTSTRAP_ADMIN_USERNAME` / `BOOTSTRAP_ADMIN_PASSWORD` values configured in `.env`
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
DATABASE_URL=sqlite:///app/data/app.db
JWT_SECRET=replace-with-at-least-32-random-characters
JWT_EXP_HOURS=24
AUTH_COOKIE_SECURE=true
TRUST_PROXY_HEADERS=true
BOOTSTRAP_ADMIN_USERNAME=replace-with-a-custom-admin-username
BOOTSTRAP_ADMIN_PASSWORD=replace-with-a-strong-first-login-password
```

Production recommendation:

```env
JWT_SECRET=replace-with-at-least-32-random-characters
AUTH_COOKIE_SECURE=true
```

`AUTH_COOKIE_SECURE=true` is the production default and requires HTTPS. Only set it to `false` for local plain-HTTP development.
`TRUST_PROXY_HEADERS=true` must only be used when the backend is reachable only through a trusted reverse proxy or the frontend container proxy.

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

## Use MySQL

SQLite is the default database. If Docker bind-mount writes are slow on your Linux server, or if you manage many nodes, switch to MySQL.

### Option 1: Use the built-in Compose MySQL container

This starts MySQL from the same `docker-compose.yml` and bind-mounts its data to `MYSQL_DATA_DIR`.

Edit `.env`:

```env
COMPOSE_PROFILES=mysql
DATABASE_URL=mysql://sublinkx:<mysql_password>@mysql:3306/sublinkx
MYSQL_IMAGE=mysql:8.4
MYSQL_DATABASE=sublinkx
MYSQL_USER=sublinkx
MYSQL_PASSWORD=replace-with-a-strong-mysql-password
MYSQL_ROOT_PASSWORD=replace-with-a-different-strong-root-password
MYSQL_DATA_DIR=./docker-data/mysql
```

Start or restart:

```bash
docker compose up -d
```

MySQL data is bind-mounted to:

```text
docker-data/mysql/
```

### Option 2: Connect to an existing local or external MySQL

If MySQL already runs on the host or another server, do not enable `COMPOSE_PROFILES=mysql`. Point the backend container to that MySQL with `DATABASE_URL`.

Host MySQL example:

```env
DATABASE_URL=mysql://sublinkx:<mysql_password>@host.docker.internal:3306/sublinkx
```

External MySQL example:

```env
DATABASE_URL=mysql://sublinkx:<mysql_password>@192.168.1.10:3306/sublinkx
```

This Compose file maps `host.docker.internal` to `host-gateway` for Linux Docker. Docker Desktop already provides the same hostname. Make sure the MySQL user can connect from the Docker subnet and that the target database exists.

Note: SQLite and MySQL are separate databases. Back up `docker-data/backend/app.db` before switching. This version does not automatically migrate SQLite data to MySQL.

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

Compose checks backend `/healthz`, and the frontend starts only after the backend is healthy. If the frontend log shows `backend could not be resolved`, check whether the backend failed first:

```bash
docker compose ps
docker compose logs --tail=200 backend
```

For Nginx Proxy Manager, 1Panel, BT Panel, or Caddy, reverse proxy:

```text
http://127.0.0.1:3000
```

### Recommended BT Panel / Outer Nginx Config

Keep the outer reverse proxy pointed at the frontend port `3000`. The frontend container will proxy backend requests to the backend container internally. Do not point an outer `/api/` location directly at `127.0.0.1:8080` unless you intentionally expose the backend port yourself.

The runtime Nginx config uses `connect-src 'self'` in its CSP. This is intentional: production deployments should keep the browser-facing frontend and API same-origin, with `/api/`, `/s/`, and `/healthz` proxied by the frontend container or by an equivalent same-origin reverse proxy rule. `script-src` additionally permits only `https://static.cloudflareinsights.com` for Cloudflare's automatically injected Web Analytics beacon. Automatic injection reports to the same-origin `/cdn-cgi/rum` endpoint, so `connect-src` does not need to be broadened. Deployments that do not enable Cloudflare Web Analytics may remove this script source from their custom Nginx configuration.

If you need a separate `/api/` location in BT Panel or Nginx, proxy it to `3000` and disable cache:

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

If you configure a separate `/s/` subscription location, disable access logs for that location so subscription tokens are not written to reverse-proxy logs:

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

It is also recommended to disable HTML cache for the frontend page, so browsers do not keep loading an old `index-*.js` bundle after upgrades:

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
