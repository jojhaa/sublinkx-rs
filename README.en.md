# sublinkx-rs

[中文](README.md)

`sublinkx-rs` is a Rust + Vue 3 project for multi-protocol, multi-client subscription management. It is a secondary modification, architectural rewrite, and feature extension based on [gooaclok819/sublinkX](https://github.com/gooaclok819/sublinkX). The project keeps the core subscription-delivery idea, while rebuilding the backend with Rust/Axum, rebuilding the admin console with Vue 3, and using SQLite as the default storage.

## Origin

- Upstream project: [gooaclok819/sublinkX](https://github.com/gooaclok819/sublinkX)
- This repository: a modified and extended Rust + Vue 3 rewrite focused on multi-protocol import, client template export, upstream template passthrough, real-link latency testing, subscription lifecycle management, and Docker deployment.
- Note: this repository is not the official upstream repository.

## Highlights

- Rust backend built with Axum, SQLx, and SQLite.
- Vue 3 admin console for nodes, subscriptions, templates, groups, settings, and exports.
- Multi-protocol node parsing for Shadowsocks, VMess, VLESS, Trojan, Hysteria2, TUIC, WireGuard, AnyTLS, and more.
- Multi-client exports for Mihomo/Clash Meta, Clash, Xray, Surge, sing-box, Quantumult X, Loon, Surfboard, Mellow, ClashR, SS SIP002/SIP008, Trojan URI, and mixed exports.
- Template-aware exports and field-fidelity checks to reduce data loss during conversion.
- Upstream subscription import and upstream Mihomo template passthrough.
- Subscription lifecycle controls including enable/disable, expiry, renewal, grouping, node filtering, and auto-detected client links.
- Real-link latency checks through a Mihomo core.
- First-login security flow with Argon2 password hashing.
- Docker deployment with local data bind mounts.

## Docker Deployment

Only one `docker-compose.yml` is needed. It uses the published Docker Hub images by default:

```text
docker.io/jojhaa/sublinkx-rs-backend:latest
docker.io/jojhaa/sublinkx-rs-frontend:latest
```

Prepare config:

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

Default fixed Docker subnet:

```env
SUBLINKX_DOCKER_SUBNET=172.31.88.0/24
```

If it conflicts with existing Docker, VPN, or LAN networks, edit `SUBLINKX_DOCKER_SUBNET` in `.env`.

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

License not specified yet. Please also review the license and attribution requirements of the upstream project [gooaclok819/sublinkX](https://github.com/gooaclok819/sublinkX) before redistribution.
