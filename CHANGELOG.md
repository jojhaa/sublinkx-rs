# Changelog

## v0.1.1 - 2026-06-23

本版本聚焦已上线环境的安全加固、Mihomo/Clash 导出稳定性、延迟测试体验和 Docker 部署可靠性。

### 安全加固

- 生产环境启动时校验 `JWT_SECRET`、默认引导账号和关键安全配置，阻止默认密钥或弱默认凭据上线。
- Web 登录态迁移到 HttpOnly Cookie + CSRF 双提交令牌，生产默认启用 Secure Cookie，并新增后端 logout 清理 Cookie。
- 登录、公开订阅、远程订阅拉取和版本检查增加限流、缓存或认证保护，降低暴力尝试、外联消耗和公开接口指纹风险。
- 修复可信代理头与真实客户端 IP 处理，非可信代理模式使用 socket peer IP，避免 XFF 伪造影响限流。
- 加强 SSRF 防护：远程订阅和延迟测试 URL 校验公网解析，阻断 DNS rebinding、IPv4-mapped IPv6 本地地址和私网目标。

### Mihomo/Clash 导出

- 修复代理名重复导致 Mihomo 报 `duplicate name` 的问题，导出和保存模板时会自动重命名并同步更新策略组引用。
- 修复模板 `include-all-proxies: true` 没有展开为当前订阅全部节点的问题。
- 无上游模板导出时生成正确的 `PROXY` select 组和 `AUTO` url-test 组，默认测速地址切换为 `https://cp.cloudflare.com/generate_204`。
- 内置 Clash/Mihomo 模板补全常用分流规则，并移除默认 GeoIP/MMDB、DNS fallback 依赖，避免客户端因无法下载数据库而整体启动失败。
- 上游订阅导入按当前目标分组查重，允许不同分组保存相同节点；自动保存的上游模板名称包含分组名。

### 延迟测试与订阅体验

- 后台自动测速新增并发设置，批量测速改为 NDJSON 流式返回，前端可实时展示每个节点的结果。
- 手动测速撞上后台测速时提示是否停止后台任务，确认后取消后台测速并自动重试。
- Mihomo 子进程增加全局并发控制、任务单飞、冷却时间和临时文件清理，降低误操作或异常任务带来的资源压力。
- 公开订阅链接新增主题化二维码，二维码中心 `RS` 标识融入码点风格。

### 部署与运维

- Docker Compose 增加后端健康检查，前端等待后端 healthy 后启动，并修复 Nginx 启动期解析 `backend` 失败的问题。
- 修复历史迁移 `0006` 校验不稳定导致线上 SQLite 报 `Migrate(VersionMismatch(6))` 的问题，默认测速 URL 变更改由后续迁移处理。
- 更新 Windows、Linux、macOS 本地运行脚本和 Docker 部署文档，补充可信反代头、同源 CSP、后端启动失败和镜像缓存排查说明。
- 已推送 Docker Hub `latest` 镜像，并额外提供后端不可变标签 `docker.io/jojhaa/sublinkx-rs-backend:20260623-migrate-fix`。

## v0.1.0 - 2026-06-01

首个公开版本，面向多协议、多客户端订阅管理场景。

### 核心能力

- Rust/Axum 后端与 Vue 3 管理控制台。
- 默认 SQLite，支持 MySQL 8.x、容器 MySQL、本机 MySQL 和外部 MySQL。
- 节点、节点分组、订阅、订阅分组、模板和系统设置管理。
- 首次登录强制修改默认账号密码，密码使用 Argon2 哈希保存。
- 中英文界面与中英文 README。

### 节点与订阅

- 支持手动多行导入、整段 Base64 自动解码、上游订阅链接导入。
- 支持 Mihomo YAML 节点提取，并可保存上游模板用于透传导出。
- 支持订阅启用/停用、到期时间、快捷续期、分组、节点筛选和自动识别客户端链接。
- 支持详细/简约两种列表展示，适配桌面、平板和移动端。

### 协议与客户端

- 支持 Shadowsocks、VMess、VLESS、Trojan、Hysteria/Hysteria2、TUIC、WireGuard、AnyTLS 等协议方向。
- 支持 Mihomo/Clash Meta、Clash、Xray、Surge、sing-box、Quantumult X、Quantumult、Loon、Surfboard、Mellow、ClashR、SS SIP002/SIP008、SSR、SSD、Trojan URI、Mixed 等导出目标。
- 增加协议 x 客户端支持矩阵，仅在大屏展示完整表格。
- 增加转换保真检查，用于对比上游 proxy 字段和二次导出字段。

### 运维与部署

- 支持 Mihomo 内核检测、下载和真实链路延迟测试。
- 保存历史延迟、最后测速时间和不可用状态。
- 提供 Docker Compose 部署，数据映射到本地目录，并支持固定 Docker 网段。
- 提供 Windows、Linux、macOS 本地开发运行脚本。

### 默认账号

```text
admin / admin123456
```

首次登录后必须修改用户名和密码。
