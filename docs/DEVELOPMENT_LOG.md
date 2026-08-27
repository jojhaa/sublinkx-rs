# Development Log

历史日志保留在根目录 `开发日志.md`。自 2026-08-27 起按本文件追加开发记录。

## 2026-08-27

- Cloudflare Web Analytics CSP 兼容：运行时 Nginx 的 `script-src` 仅新增 `https://static.cloudflareinsights.com`，允许 Cloudflare 自动注入的 Beacon 加载；保留 `connect-src 'self'`，由自动注入模式通过本站 `/cdn-cgi/rum` 上报，不放开 `unsafe-inline`、通配符或额外连接域名。
- Windows 验证：前端 `v0.2.0` 生产构建通过，生产依赖审计为 0 个漏洞；使用官方 `nginx:1.29-alpine` 验证配置语法成功。
- Docker 与生产部署：前端 `v0.2.0-csp-hotfix.1` 和 `latest` 均指向 `sha256:d5ada9eb85e61bff9f136f864cecaef75d0ad88fb06486d298f750a42ac1be92`，包含 `linux/amd64`；服务器先保留 `rollback-before-v0.2.0-csp-hotfix.1` 本地回滚标签，再仅执行前端拉取和 `--no-deps` 重建，后端、SQLite、MySQL、密钥和数据挂载均未改动。
- 生产验收：公网首页返回的新 CSP 已覆盖 Cloudflare 实际注入的版本化 Beacon URL；真实 Chrome 控制台为 0 错误、0 警告，同源 `/cdn-cgi/rum` POST 返回 204；前后端容器均运行且重启次数为 0，健康检查和 `0.2.0` 版本接口保持 200。
- 依赖与知识产权：本次未引入 Cloudflare 脚本文件或新依赖，脚本由站点管理员已启用的 Cloudflare Web Analytics 在边缘自动注入；使用边界依据 Cloudflare 官方 CSP 文档，未新增字体、图片、图标、音视频或需归档许可证的本地资源。

- `v0.2.0` 性能优化：节点、普通订阅、模板和上游订阅列表改为服务端分页与数据库筛选；普通订阅关联节点和节点分组改为批量读取，列表不再重复返回完整节点对象；新增总览统计接口和模板类型聚合统计。
- 设置读取由 7 次逐键查询合并为 1 次批量查询；上游历史来源回填从每次列表 GET 移到服务启动阶段单次执行；新增、更新、移动和删除后只刷新相关列表，减少重复响应。
- 兼容边界：本次没有新增数据库迁移，不修改现有节点、订阅、订阅 Token、JWT 密钥或加密数据；分页 API 未传参数时默认第 1 页、每页 20 条，最大每页 1000 条。
- Windows 自动验证：后端 59 项测试全部通过，`cargo fmt --check` 和 Clippy `-D warnings` 通过；前端 `v0.2.0` 生产构建通过，生产依赖审计为 0 个漏洞。
- Windows 真实浏览器验收：使用隔离 SQLite 临时数据库完成首次登录、统计总览、节点分页、订阅轻量列表、模板两页与类型筛选、上游订阅和设置页面验证；覆盖 `1280x720` 与 `390x844`，验收数据和浏览器临时产物已清理。
- 依赖与知识产权：本次未新增依赖、字体、图片、图标、音视频、第三方代码或外部文案，无新增署名、NOTICE、源代码提供或素材授权要求。
- GitHub 发布：提交 `7b3a612001d1e54e12e6aea06ccf408f3ef0d1a4` 已推送到 `main`，带注释标签 `v0.2.0` 指向该提交，并创建中文 Release：`https://github.com/jojhaa/sublinkx-rs/releases/tag/v0.2.0`。
- Docker Hub 发布：后端 `0.2.0`、`v0.2.0`、`latest` 均指向 `sha256:45fab4f3b5c322f4164f663bc92d31ee1541b9204e66afe111d3931dfebaf4e9`；前端三个标签均指向 `sha256:358896ad4ddc43d1a3d7aba4670bb78ccd7e9539a4ffe54aeef9f5a1ea5fa22b`，远端清单包含 `linux/amd64`。后端标准运行层构建受 Debian 官方软件源连接失败影响，发布时复用已验证的 `v0.1.9` 官方运行层并仅替换经测试的 `v0.2.0` 后端二进制，临时 Dockerfile 未提交且已删除。
- Linux 生产部署：先只读确认 `/data/sublinkx-rs` 的容器、镜像、挂载和实际数据库配置；生产业务实际使用 `sqlite:///app/data/app.db`，MySQL 容器中没有 SublinkX 业务表。本次仅执行 `docker compose pull backend frontend` 和 `docker compose up -d --no-deps backend frontend`，未停止或重建 MySQL，未改动数据挂载和密钥配置。
- 生产备份：部署前通过 SQLite 在线备份 API 创建 `/data/sublinkx-rs/backups/app-before-v0.2.0-20260827T100244Z.db`，完整性为 `ok`，SHA-256 为 `fde97ec8b2f6481c57434b12114812cf8de564a02bd62dd2a2613f37bad4baa4`；服务器保留前后端 `rollback-before-v0.2.0` 本地回滚镜像标签。
- 生产数据与接口验收：部署前后均为节点 260、普通订阅 13、模板 75、上游订阅 4，迁移记录 17 条且最大版本 17，SQLite 完整性为 `ok`；前后端容器持续运行且重启次数为 0；本机反代和 `https://sub.rwyyd.xyz/`、`/healthz`、`/api/v1/version` 均返回 200，版本接口返回 `0.2.0`，未认证分页节点请求返回预期 401。
- 生产已知风险：日志发现 1 次 Mihomo 测速子进程临时端口 `45867` 占用，未引发后端退出或容器重启，发布后需继续观察后台与手动测速并发场景。

- 扩展 Mihomo YAML 上游订阅解析，新增 VMess、TUIC、WireGuard 和 AnyTLS 转换，连同既有 Shadowsocks、VLESS、Trojan、Hysteria2 共覆盖 8 类当前可完整导出的协议。
- TUIC 同时支持 v4 token 和 v5 UUID/password，保留 SNI、ALPN、证书校验、拥塞控制、UDP 中继及常用连接参数。
- AnyTLS 保留 SNI、ALPN、客户端指纹、空闲会话参数、证书校验以及 ShadowTLS、ResTLS、JLS 扩展配置；不引入 AnyTLS + Reality。
- WireGuard 支持简化配置和单 peer 完整配置，保留 IPv4/IPv6、密钥、Allowed IPs、reserved、keepalive、DNS 和 MTU；多 peer 配置会明确失败，避免只取第一个 peer 后静默损坏。
- 未支持或字段不完整的 Mihomo 节点现在进入导入失败明细，不再静默跳过；同步遇到失败时继续沿用现有保护，不停用旧节点。
- 新增混合 Mihomo YAML、AnyTLS 完整字段、TUIC v4 和 WireGuard 多 peer 防截断测试。
- Windows 验证：`cargo test --locked` 53 项通过，`cargo clippy --locked --all-targets -- -D warnings` 通过，前端生产构建通过，`git diff --check` 通过。
- 未执行：真实供应商上游订阅导入、Mihomo 二进制加载测试、Linux Docker/生产环境验收。
- 依赖与知识产权：未新增依赖、字体、图片、图标、音视频、第三方代码或文案；字段映射依据 Mihomo 官方配置文档独立实现，无新增署名、NOTICE 或源代码提供要求。
- Docker 发布：已构建并推送 `docker.io/jojhaa/sublinkx-rs-backend:latest`（`sha256:152ca60cdd18bfb26485e0d0a64b2fb2036e47624fe79e252e7ecae5fc793ba5`）和 `docker.io/jojhaa/sublinkx-rs-frontend:latest`（`sha256:e26a9ea66e6ce4f6fa9718ecca3aee301dde231e42788278042fa10ed63cf8b4`），远端清单均确认为 `linux/amd64`；未覆盖 `0.1.7`/`v0.1.7` 固定标签，未执行 Linux 生产部署验收。
- `v0.1.8` 版本准备：后端 `Cargo.toml/Cargo.lock` 与前端 `package.json/package-lock.json` 已统一更新为 `0.1.8`，`CHANGELOG.md` 新增 Mihomo YAML 多协议导入中文更新说明；尚未创建 Git 标签、GitHub Release 或 Docker 固定版本镜像。
- `v0.1.8` 验证：后端 53 项测试通过，Clippy `-D warnings` 通过；前端以 `frontend@0.1.8` 完成生产构建，生产依赖审计为 0 个漏洞；未执行真实上游订阅与 Linux 生产环境验收。
- Docker `v0.1.8` 发布：后端 `0.1.8`、`v0.1.8`、`latest` 均指向 `sha256:5aece9243cc08342e31ad378fa36fba7f83f12a4227ebc466f8db64ede3b717b`；前端三个标签均指向 `sha256:8dfe5334b25d6c3ea96fcf745a0d1bcb5f3f41c7e6c5d9fd92cb28311297b259`。六个远端标签均核验为 `linux/amd64`；因 Debian 官方软件源和 Docker Hub 上传超时，后端发布复用已验证的官方 Debian 固定摘要与构建缓存，临时发布 Dockerfile 已删除。尚未执行 Linux 生产部署验收。
- Trojan 修复：定位并修复 Mihomo YAML 转换链路中的百分号编码密码未解码和 Trojan 错误输出 `servername` 问题；新增 Reality、完整 WS/gRPC、证书与 TLS 扩展、SMUX 及常用网络字段保真。后端测试增至 56 项并全部通过，Clippy `-D warnings` 通过，项目自带 Mihomo 核心成功加载包含特殊密码、`sni`、WS 和 SMUX 的回归配置。未使用真实 Trojan 节点联网验收；旧节点需从上游重新同步后再测速。
- `v0.1.9` 版本与 Docker 发布：后端、前端及锁文件版本统一为 `0.1.9`，`CHANGELOG.md` 新增 Trojan 兼容性中文更新说明；仅发布 `v0.1.9` 和 `latest`，未创建纯数字 `0.1.9` 标签。
- Docker Hub 远端核验：后端 `v0.1.9` 与 `latest` 均指向 `sha256:62c53048117c9a138e73e1de1fd4950e1de2002eb6f1d1b45c0c2ce25db23d00`；前端两个标签均指向 `sha256:d77981e811df569d11c5ec1fd7d0e616ee03afa93cbc5d35313d9f117429904c`；均包含 `linux/amd64` 清单，两个仓库的纯数字 `0.1.9` 标签均确认不存在。
- `v0.1.9` 验证：`cargo test --locked` 56 项通过，`cargo clippy --locked --all-targets -- -D warnings` 与 `cargo fmt --check` 通过；前端生产构建通过，生产依赖审计为 0 个漏洞。尚未在 Linux 生产服务器拉取运行，也未使用真实 Trojan 节点完成联网测速验收。
- 本次发布未新增依赖、字体、图片、图标、音视频、第三方代码或外部文案，无新增署名、NOTICE、源代码提供或素材授权要求；未新增数据库迁移。
- GitHub `v0.1.9` 发布：发布提交 `667abf15e9418d508282e374712eaf1026bdb638` 已推送到 `main`，带注释标签 `v0.1.9` 指向该提交，并创建中文 Release：`https://github.com/jojhaa/sublinkx-rs/releases/tag/v0.1.9`。
