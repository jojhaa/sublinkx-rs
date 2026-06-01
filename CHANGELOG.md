# Changelog

## v0.1.0 - 2026-06-01

首个公开版本。

### 新增

- Rust/Axum 后端与 Vue 3 管理控制台。
- 节点、节点分组、订阅、订阅分组、模板和系统设置管理。
- SQLite 默认数据库，并支持 MySQL 8.x。
- 多协议导入，包括手动多行导入、整段 Base64 解码、上游订阅导入和 Mihomo YAML 节点提取。
- 多客户端导出，包括 Mihomo/Clash、Xray、Surge、sing-box、Quantumult X、Loon、Surfboard、Mellow、ClashR、SS SIP002/SIP008、Trojan URI 等目标规划与部分实现。
- 上游 Mihomo 模板透传，减少复杂分流规则二次转换损坏。
- 转换保真检查，用于对比上游 proxy 字段和二次导出字段。
- Mihomo 内核管理和真实链路延迟测试。
- 首次登录强制修改默认账号密码，密码使用 Argon2 哈希保存。
- Docker Compose 部署，支持本地数据映射、固定 Docker 网段、容器 MySQL 或宿主机 MySQL。
- 中英文 README、Docker 部署文档、兼容矩阵、协议 x 客户端矩阵和模板说明。

### 默认账号

```text
admin / admin123456
```

首次登录后必须修改用户名和密码。
