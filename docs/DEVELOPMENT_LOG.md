# Development Log

历史日志保留在根目录 `开发日志.md`。自 2026-08-27 起按本文件追加开发记录。

## 2026-08-27

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
