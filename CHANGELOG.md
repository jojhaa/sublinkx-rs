# Changelog

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
