# sublinkx-rs

中文 | [English](#english)

`sublinkx-rs` 是一个面向多协议、多客户端订阅管理的 Rust + Vue 3 重构项目。它以 SQLite 作为默认数据存储，后端负责节点解析、订阅导出、模板渲染和延迟检测，前端提供节点、订阅、模板、分组和系统设置的管理界面。

## 项目特性

- Rust 后端：基于 Axum、SQLx 和 SQLite，适合轻量部署到 Linux 服务器。
- Vue 3 前端：提供紧凑的管理控制台，支持节点、订阅、模板、分组和设置管理。
- 多协议节点：支持 Shadowsocks、VMess、VLESS、Trojan、Hysteria2、TUIC、WireGuard、AnyTLS 等协议的解析和导出扩展。
- 多客户端导出：覆盖 Mihomo/Clash Meta、Clash、Xray、Surge、sing-box、Quantumult X、Loon、Surfboard、Mellow、ClashR、SS SIP002/SIP008、Trojan URI 等目标。
- 模板与保真检查：支持 Clash/Mihomo、Surge、sing-box、Quantumult X 等模板方向，并提供上游字段与二次导出字段的完整性检查。
- 上游订阅导入：支持从上游订阅链接导入节点，也支持上游模板透传，避免复杂配置被二次转换破坏。
- 订阅生命周期：支持启用、停用、到期时间、续期、分组、节点筛选和客户端自动识别链接。
- 真实延迟测试：可通过 Mihomo 内核测试真实链路延迟，保存历史延迟、最后测速时间和不可用状态。

## 目录结构

```text
sublinkx-rs/
  backend/    Rust API service, SQLite migrations, export/render logic
  frontend/   Vue 3 + Vite admin console
  docs/       Planning docs, compatibility matrix, template notes
```

## 快速启动

后端：

```powershell
cd backend
cargo run
```

前端：

```powershell
cd frontend
npm install
npm run dev -- --host 127.0.0.1 --port 4173
```

默认访问地址：

- 前端：http://127.0.0.1:4173
- 后端：http://127.0.0.1:8080

默认管理员账号来自后端环境变量：

- `BOOTSTRAP_ADMIN_USERNAME`
- `BOOTSTRAP_ADMIN_PASSWORD`

未设置时会使用开发默认值，请在正式部署前修改。

## Mihomo 内核

真实链路延迟测试依赖 Mihomo/Clash Meta 内核。生产部署时请将对应平台的内核二进制放到：

```text
backend/mihomo/
```

例如 Windows 可放 `backend/mihomo/mihomo.exe`，Linux 可放 `backend/mihomo/mihomo`，也可以在网页设置页中指定自定义内核路径。

## 文档

- [重构蓝图](docs/plan.md)
- [客户端兼容矩阵](docs/client-compatibility.md)
- [协议 x 客户端矩阵](docs/protocol-client-matrix.md)
- [客户端目标注册表](docs/client-target-registry.md)
- [Clash 分流模板说明](docs/clash-routing-template.md)

## English

`sublinkx-rs` is a Rust + Vue 3 rewrite for multi-protocol, multi-client subscription management. It uses SQLite by default and provides a lightweight backend for node parsing, subscription export, template rendering, upstream import, and real-link latency testing, plus a Vue 3 admin console.

## Highlights

- Rust backend: built with Axum, SQLx, and SQLite for simple Linux deployment.
- Vue 3 frontend: compact admin UI for nodes, subscriptions, templates, groups, and settings.
- Multi-protocol nodes: designed for Shadowsocks, VMess, VLESS, Trojan, Hysteria2, TUIC, WireGuard, AnyTLS, and more.
- Multi-client exports: targets Mihomo/Clash Meta, Clash, Xray, Surge, sing-box, Quantumult X, Loon, Surfboard, Mellow, ClashR, SS SIP002/SIP008, Trojan URI, and mixed exports.
- Template-aware exports: supports client-specific templates and field fidelity checks to prevent accidental data loss during conversion.
- Upstream subscription import: import nodes from upstream links or preserve upstream Mihomo templates for passthrough subscriptions.
- Subscription lifecycle: enable/disable, expiry time, renewal, grouping, node filtering, and auto-detected client links.
- Real-link latency checks: runs tests through a Mihomo core and stores latency, last test time, and failure status.

## Quick Start

Backend:

```powershell
cd backend
cargo run
```

Frontend:

```powershell
cd frontend
npm install
npm run dev -- --host 127.0.0.1 --port 4173
```

Default URLs:

- Frontend: http://127.0.0.1:4173
- Backend: http://127.0.0.1:8080

Admin bootstrap credentials are read from:

- `BOOTSTRAP_ADMIN_USERNAME`
- `BOOTSTRAP_ADMIN_PASSWORD`

Change the development defaults before production deployment.

## Mihomo Core

Real-link latency testing requires a Mihomo/Clash Meta binary. Put the platform-specific binary under:

```text
backend/mihomo/
```

For example, use `backend/mihomo/mihomo.exe` on Windows or `backend/mihomo/mihomo` on Linux. A custom path can also be configured from the settings page.

## License

License not specified yet.
