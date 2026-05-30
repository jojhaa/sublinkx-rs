# sublinkx-rs

[中文](README.md)

`sublinkx-rs` is a Rust + Vue 3 project for multi-protocol, multi-client subscription management. It is a secondary modification, architectural rewrite, and feature extension based on [gooaclok819/sublinkX](https://github.com/gooaclok819/sublinkX). The project keeps the core subscription-delivery idea, while rebuilding the backend with Rust/Axum, rebuilding the admin console with Vue 3, and using SQLite as the default storage.

## Origin

- Upstream project: [gooaclok819/sublinkX](https://github.com/gooaclok819/sublinkX)
- This repository: a modified and extended Rust + Vue 3 rewrite focused on multi-protocol import, client template export, upstream template passthrough, real-link latency testing, subscription lifecycle management, and Docker deployment.
- Note: this repository is not the official upstream repository.

## Acknowledgements

Thanks to [gooaclok819/sublinkX](https://github.com/gooaclok819/sublinkX) for the original subscription-delivery idea and implementation reference. `sublinkx-rs` started from that project direction and then evolved into a Rust + Vue 3 rewrite with additional operational features. Please respect the upstream project's open-source contribution and license requirements when using, modifying, or redistributing this project.

## What This Version Adds

| Area | Upstream project | sublinkx-rs additions |
| --- | --- | --- |
| Stack | Original implementation | Backend rewritten with Rust/Axum, frontend rewritten with Vue 3, SQLite as default storage |
| Admin console | Core subscription management | Full console for nodes, subscriptions, templates, groups, settings, and language switching, with desktop, tablet, and mobile layouts |
| Node import | Basic subscription import | Manual multi-line import, full Base64 subscription decoding, upstream URL import, and Mihomo YAML proxy extraction |
| Upstream templates | Mostly conversion-oriented | Upstream Mihomo template passthrough for subscriptions that should not be converted twice |
| Client exports | Original client adaptation idea | Extended targets including Mihomo/Clash Meta, Clash, Xray, Surge, sing-box, Quantumult X, Loon, Surfboard, Mellow, ClashR, SS SIP002/SIP008, Trojan URI, and mixed exports |
| Template system | Basic template direction | Template management plus Clash/Mihomo routing templates and renderers for Surge, sing-box, Quantumult X, and more |
| Fidelity checks | Manual validation after export | Field-fidelity checks that compare upstream proxy fields against second-pass exported fields |
| Subscription lifecycle | Basic distribution | Enable/disable, expiry time, quick renewal, groups, node filtering, and auto-detected client links |
| Latency testing | Not a core focus | Real-link latency checks through Mihomo, with persisted latency, test time, and failure status |
| Security | Basic account configuration | Forced first-login credential change and Argon2 password hashing |
| Deployment | Mostly manual deployment | Docker Hub images, a single `docker-compose.yml`, local data bind mounts, and a fixed Docker subnet |
| Documentation | Upstream documentation | Separate Chinese/English README files, Docker deployment docs, compatibility matrices, and protocol x client design notes |

## Highlights

- Rust backend built with Axum, SQLx, and SQLite.
- Vue 3 admin console for nodes, subscriptions, templates, groups, settings, and exports.
- Responsive console layouts for desktop, tablet, and mobile.
- Detailed and compact list modes for nodes and subscriptions. Compact node cards focus on name and real-link latency; compact subscription cards focus on name and expiry time, with details opened in a modal.
- Page-size preferences for node and subscription lists are persisted in the browser.
- Multi-protocol node parsing for Shadowsocks, VMess, VLESS, Trojan, Hysteria2, TUIC, WireGuard, AnyTLS, and more.
- Multi-client exports for Mihomo/Clash Meta, Clash, Xray, Surge, sing-box, Quantumult X, Loon, Surfboard, Mellow, ClashR, SS SIP002/SIP008, Trojan URI, and mixed exports.
- Template-aware exports and field-fidelity checks to reduce data loss during conversion.
- Upstream subscription import and upstream Mihomo template passthrough.
- Subscription lifecycle controls including enable/disable, expiry, renewal, grouping, node filtering, and auto-detected client links.
- Real-link latency checks through a Mihomo core.
- First-login security flow with Argon2 password hashing.
- Docker deployment with local data bind mounts.
- SQLite by default, with optional MySQL 8.x support.

## Docker Deployment

Only one `docker-compose.yml` is needed. It uses the published Docker Hub images by default:

```text
docker.io/jojhaa/sublinkx-rs-backend:latest
docker.io/jojhaa/sublinkx-rs-frontend:latest
```

Prepare config:

Clone the deployment configuration from GitHub first:

```bash
git clone https://github.com/jojhaa/sublinkx-rs.git
cd sublinkx-rs
```

This provides `docker-compose.yml`, `.env.example`, and the deployment docs. Docker still pulls the published Docker Hub images by default.

```bash
cp .env.example .env
```

For production, edit `.env` and set a strong secret:

```env
JWT_SECRET=use-a-long-random-secret
```

Start:

```bash
docker compose up -d
```

Open:

```text
http://localhost:3000
```

Default first login:

```text
admin / admin123456
```

The first login must change both username and password.

Runtime data is bind-mounted to local folders by default:

```text
docker-data/
  backend/
    app.db
  mihomo/
    mihomo
```

SQLite is the default database. If SQLite on a Linux bind mount is slow, switch to MySQL:

Use the built-in Compose MySQL container:

```env
COMPOSE_PROFILES=mysql
DATABASE_URL=mysql://sublinkx:sublinkx_password@mysql:3306/sublinkx
MYSQL_PASSWORD=change-this-password
MYSQL_ROOT_PASSWORD=change-this-password
```

Use an existing host or external MySQL:

```env
DATABASE_URL=mysql://sublinkx:change-this-password@host.docker.internal:3306/sublinkx
```

For full MySQL container, host MySQL, and external MySQL examples, see [Docker Deployment](docs/docker.en.md).

Then start:

```bash
docker compose up -d
```

Default fixed Docker subnet:

```env
SUBLINKX_DOCKER_SUBNET=172.31.88.0/24
```

If it conflicts with existing Docker, VPN, or LAN networks, edit `SUBLINKX_DOCKER_SUBNET` in `.env`.

### Reverse Proxy Notes

The Docker deployment exposes only the frontend container on host port `3000`. The backend `8080` port is internal to the Docker network by default. For BT Panel, 1Panel, Nginx Proxy Manager, Caddy, or an outer Nginx reverse proxy, point the site to:

```text
http://127.0.0.1:3000
```

The frontend container then proxies backend paths internally:

```text
/api/     -> backend:8080/api/
/s/       -> backend:8080/s/
/healthz  -> backend:8080/healthz
```

If you create a separate outer `/api/` rule, still proxy it to `3000` and disable cache:

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

More details: [Docker Deployment](docs/docker.en.md).

## Local Development

Backend:

```powershell
cd backend
cargo run
```

Frontend:

```powershell
cd frontend
npm install
npm run dev -- --host 127.0.0.1 --port 5173
```

Default URLs:

- Frontend: http://127.0.0.1:5173
- Backend: http://127.0.0.1:8080

## Documentation

- [Docker Deployment](docs/docker.en.md)
- [Refactor Plan](docs/plan.md)
- [Client Compatibility Matrix](docs/client-compatibility.md)
- [Protocol x Client Matrix](docs/protocol-client-matrix.md)
- [Client Target Registry](docs/client-target-registry.md)
- [Clash Routing Template](docs/clash-routing-template.md)

## License

This project is licensed under the [MIT License](LICENSE).

This project is a secondary modification and rewrite based on [gooaclok819/sublinkX](https://github.com/gooaclok819/sublinkX), which is also licensed under the MIT License. Thanks to the upstream author for the open-source contribution. When using, modifying, or distributing this project, please keep the copyright and license notices for both this project and the upstream project.
