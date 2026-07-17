# sublinkx-rs 重构蓝图

## 1. 目标

将旧版 `sublinkX-2.1` 的订阅分发思路重构为一个可维护、可部署、可扩展的多协议订阅管理平台。

核心目标：

- 支持更多节点协议，避免每新增协议都大规模改造。
- 拆分协议解析、数据存储、导出渲染和前端编辑。
- 使用明确的认证、密码哈希和 token 边界替代弱安全设计。
- 保持产品重点：节点管理、订阅管理、模板管理和客户端导出。

## 2. 总体架构

```text
sublinkx-rs/
  backend/
  frontend/
  docs/
```

后端技术栈：

- Rust
- Axum
- SQLx
- SQLite first，支持 MySQL
- Serde
- JsonWebToken
- Argon2
- Reqwest
- Tracing

前端技术栈：

- Vue 3
- Vite
- TypeScript
- Pinia
- Vue Router

## 3. 与旧系统的关键区别

旧系统特点：

- 以功能脚本为主组织代码。
- 节点模型更依赖原始链接字符串。
- 新增协议时容易出现大量分支判断。
- 账号安全设计较弱。
- 订阅 token 容易被预测或固定化。

新系统方向：

- 以领域模型组织代码。
- 使用结构化节点模型保存协议元数据。
- 使用协议注册表扩展协议。
- 密码使用 Argon2 哈希保存。
- 订阅 token 随机生成并支持轮换。
- 对远程拉取、模板渲染和公开订阅访问设置边界。

## 4. 目录规划

### 4.1 后端

```text
backend/
  src/
    main.rs
    app.rs
    state.rs
    api/
    config/
    db/
    domain/
    dto/
    errors/
    middleware/
    protocols/
    repository/
    services/
    utils/
  migrations/
```

### 4.2 前端

```text
frontend/
  src/
    api/
    components/
    layouts/
    router/
    store/
    styles/
    types/
    utils/
    views/
```

## 5. 数据模型原则

节点数据应包含：

- 稳定的公共字段
- `protocol` 协议类型
- `settings_json` 协议专用字段
- `raw_link` 原始链接，便于兼容和追踪
- `fingerprint` 去重指纹
- `source_type` 和 `source_ref` 导入来源

订阅数据应包含：

- 随机 token
- 节点排序关系表
- 可选默认客户端
- 可选模板绑定
- 启用/停用状态
- 到期时间

模板数据应优先保存到数据库。只有出现明确需求时，才额外写入文件系统。

## 6. 协议扩展策略

后端必须使用协议注册表模型。每个协议模块应提供：

- 链接解析器
- 结构化校验器
- 能力声明
- 按客户端类型声明渲染支持

规划中的协议模块结构：

```text
backend/src/protocols/
  mod.rs
  traits.rs
  registry.rs
  ss.rs
  ssr.rs
  vmess.rs
  vless.rs
  trojan.rs
  hysteria.rs
  hysteria2.rs
  tuic.rs
  wireguard.rs
```

设计规则：

- 新增一个协议时，不应要求修改大量无关业务逻辑。

## 7. 客户端兼容策略

新项目需要面向当前主流客户端，而不是只保留旧版输出名称。

主要目标家族：

- Mihomo / Clash 家族客户端
- v2rayN / Xray 家族客户端
- Surge 家族客户端
- sing-box 家族客户端

详细规则见：

- [客户端兼容矩阵](client-compatibility.md)
- [客户端目标注册表](client-target-registry.md)
- [协议 x 客户端矩阵](protocol-client-matrix.md)

## 8. 安全基线

必须具备：

- 密码使用 Argon2 保存。
- 管理后台使用 JWT 鉴权。
- 公开订阅 token 与后台登录 token 分离。
- 订阅 token 必须随机生成并可轮换。
- 远程拉取需要限制超时、大小、协议和目标。
- 模板操作必须限制在可信边界内。

## 9. 分阶段开发计划

### Phase 0：规划

交付内容：

- 项目骨架
- 重构蓝图
- 数据模型设计
- 协议扩展规则

### Phase 1：后端基础

交付内容：

- Rust 工程初始化
- Axum 应用启动
- 配置加载
- tracing 和错误中间件
- SQLx 初始化和第一批迁移
- 用户模型和管理员引导

### Phase 2：认证与后台基础

交付内容：

- 登录 API
- JWT 中间件
- 当前用户 API
- 修改密码 API

### Phase 3：节点领域

交付内容：

- 节点分组 CRUD
- 节点 CRUD
- 原始链接导入
- 解析为结构化协议设置
- 使用 fingerprint 去重

### Phase 4：订阅领域

交付内容：

- 订阅 CRUD
- 绑定节点并维护排序
- 轮换订阅 token
- 启用或停用订阅
- 到期时间和续期

### Phase 5：渲染与导出

交付内容：

- 统一规范化节点管线
- Mihomo 导出器
- Surge 导出器
- Xray URI bundle 导出器
- sing-box outbound 导出器
- 客户端家族兼容过滤

### Phase 6：前端管理台

交付内容：

- 登录页
- 总览页
- 节点管理
- 订阅管理
- 模板管理
- 系统设置

### Phase 7：加固

交付内容：

- 访问日志
- 远程拉取保护
- 协议解析测试
- Docker 部署文件

## 10. MVP 范围

第一版可用范围：

- 管理员登录
- 节点 CRUD
- 订阅 CRUD
- 节点排序
- Mihomo / Surge / Xray 导出
- 随机 token 订阅访问

首个里程碑不包含：

- 多角色权限系统
- 高级统计面板
- 复杂模板继承
- 后台同步调度器

## 11. 后续实现顺序

推荐顺序：

1. 稳定 Rust 后端基础。
2. 完善 SQLx 迁移。
3. 定义节点和订阅核心领域类型。
4. 稳定 Vue 3 前端基础布局。
5. 优先打通认证流程。
6. 再推进协议扩展和多客户端导出。

## 12. 不可妥协的设计规则

- 不要把协议解析耦合到 controller handler。
- 不要保存明文密码。
- 不要使用可预测订阅 token。
- 不要让协议专用字段膨胀成一张巨型表。
- 不要混用后台鉴权和公开订阅访问。

## 13. 安全修复进度

2026-06-08：
- P0 已完成：增加生产环境 JWT/default bootstrap 凭据保护。
- P1 已完成：增加登录限流、远程订阅拉取 SSRF/大小/TLS 保护、导出换行注入保护、订阅节点事务更新、MySQL 旧库追加式 schema 升级；生产认证 Cookie 默认启用 `Secure`，网页登录/改密响应不再向 JavaScript 暴露 Bearer access token。
- P1 追加修复：登录限流改为优先使用可信代理重写的 `X-Real-IP`，并在缺失时取 XFF 链末端；容器和部署文档中的 Nginx XFF 示例改为覆盖 `$remote_addr`，避免客户端伪造限流 IP；远程订阅拉取改为使用已校验 DNS 解析结果 pin 住 reqwest 连接，关闭 DNS rebinding SSRF 窗口。
- P2 已完成：增加延迟测试 URL 的公网解析校验，移除 Docker MySQL 默认弱密码，同步修正部署文档中的弱凭据示例，脱敏订阅 token 请求日志，限制 Mihomo core 配置路径和自动下载大小，限制 Mihomo 下载来源，将前端 JWT 持久化从 localStorage 收窄到 sessionStorage，增加 JWT token_version 失效机制，默认不信任可伪造的代理 IP 头，关闭容器和外层 Nginx `/s/` 订阅入口 access log，补充基础安全响应头，并升级网页登录态为 HttpOnly Cookie + CSRF 双提交令牌，同时保留 Bearer token 兼容；新增后端 logout 清理 HttpOnly Cookie，并补充 CSP 同源部署和可信反代头说明。
- P2 追加修复：限制 `JWT_EXP_HOURS` 为 1 到 720 小时并安全转换 JWT `exp`；认证 Cookie 改为浏览器会话 Cookie，让 HttpOnly Cookie 与 sessionStorage CSRF token 生命周期一致；管理台导出预览改为 Blob 临时链接打开，避免弹窗失败时覆盖当前页面；前端 API 默认同源，只有本地开发显式配置才指向独立后端。
- P3 已完成：新增已登录管理台专用订阅导出入口 `/api/v1/subscriptions/{id}/export`，前端预览改用 CSRF 认证 Blob 打开，避免浏览器历史和管理台预览链接出现 `/s/{token}`；订阅列表 token 改为脱敏显示，导出响应增加 no-store/no-referrer 头。
- 功能增强：订阅管理台新增公开订阅链接二维码，二维码在浏览器本地生成，不依赖外部二维码服务；二维码中心增加 `RS` 品牌标识并使用高纠错级别。
- 视觉优化：订阅二维码弹窗改为贴合管理台浅色玻璃和青绿色仪表盘主题，二维码本体改为自绘圆角码点、定制定位角和中心模块化 `RS`，并增加主题化展示框、角标和链接信息区。
- 功能增强：整理本地运行入口，Windows 复用 `scripts/dev.ps1`/`scripts/dev.cmd`，Linux 和 macOS 复用 `scripts/dev.sh`，并在中英文 README 中同步运行方式。
- 功能修复：`scripts/dev.ps1` 启动前增加后端和前端端口占用检查，前后端日志写入 `.dev-logs/`，并修复 Windows PowerShell 将 `cargo run` 正常 stderr 输出误包装为错误对象的问题，避免本地开发窗口直接关闭且无法定位原因。
- 安全追加修复：`/api/v1/version/update-check` 改为登录后访问并增加服务端 10 分钟缓存；公开 `/api/v1/version` 降敏，不再返回环境、系统架构、时区和 uptime；logout 改为要求有效会话和 CSRF；批量/自动/单节点延迟测试增加全局单飞、冷却和 Mihomo 子进程并发限制。
- 安全追加修复：公开订阅导出 `/s/{token}` 增加 IP/全局轻量限流和 30 秒短 TTL 响应缓存；登录限流在非可信代理模式下改用真实 peer IP；改密允许只修改密码；远程订阅导入增加 1000 节点上限和批量 fingerprint 查重；About 页移除已降敏的服务器环境字段；Mihomo 延迟测试临时配置改为 guard 清理；SSRF 校验补充 IPv4-mapped IPv6 本地地址阻断。
- 功能增强：后台自动测速新增并发数设置 `latency.concurrency`，默认 2、范围 1-8；自动测速和批量测速按该设置并发执行，同时保留全局 Mihomo 子进程上限。
- 功能修复：后台自动测速不再占用手动测速单飞锁；自动测速运行期间手动测速会等待全局 Mihomo 并发许可，避免前端点击时报 429。
- 功能修复：手动测速撞上后台自动测速时，后端返回专用 `latency_auto_running` 冲突，前端提示是否停止后台测速；确认后调用后台测速取消接口并自动重试手动测速，后台任务收到取消信号后停止继续派发并尽快杀掉正在运行的 Mihomo 子进程。
- 功能优化：节点批量测速新增 NDJSON 流式接口，前端批量测速改为边读边更新节点结果，不再等待整批全部完成后一次性刷新；内置 Nginx 和部署文档为该流式接口关闭代理缓冲。
- 功能修复：远程订阅导入的重复节点改为按目标分组查重，不再因其他分组存在相同 fingerprint 而跳过；保存上游 Mihomo 模板时模板名称带上分组名；Mihomo 导出会自动修正重复 `proxies[].name` 并同步更新 `proxy-groups[].proxies` 引用，避免客户端报 `duplicate name`；Mihomo YAML 保存模板前也会执行相同去重，覆盖非上游模板导出路径。
- 验证追加：后端 Docker/Linux release 构建通过；Mihomo 代理名去重目标单测通过；Mihomo 保存模板去重目标单测通过；diff whitespace 检查通过。本机 `cargo check` 因缺少 MSVC `link.exe` 未能运行到项目代码阶段。
- 编译追加：使用 `DOCKER_REGISTRY=docker.m.daocloud.io` 完成生产后端和前端镜像编译，产物为 `jojhaa/sublinkx-rs-backend:latest` 与 `jojhaa/sublinkx-rs-frontend:latest`。
- 发布追加：已将后端和前端生产镜像推送到 Docker Hub `latest` 标签；Docker CLI 通过覆盖推送更新远端标签，未执行远端标签删除以避免短暂拉取失败窗口。
- 配置修复：本地 `.env` 替换默认/过短 `JWT_SECRET` 和默认首次引导密码，并补齐 MySQL profile 插值所需密码变量；`docker compose config --quiet` 校验通过。
- 功能修复：Mihomo/Clash YAML 导出会将模板中的 `include-all-proxies: true` 显式展开为当前订阅的全部节点，并避免重复追加同名 `AUTO` 策略组，修复已选择大量节点但客户端策略组只显示少量节点的问题。
- 部署修复：前端 Nginx 改为通过 Docker 内置 DNS 运行时解析 `backend`，避免后端未就绪时 Nginx 因启动期解析失败报 `host not found in upstream "backend"` 并退出。
- 验证已完成：后端测试、后端 Clippy 严格检查、前端生产构建、diff whitespace 检查均通过。
- 发布追加：重新编译并覆盖推送 Docker Hub `latest` 镜像；后端 digest 为 `sha256:3ca280fae582589881c102820fc2e0b4c91247618d5438f397f0cb2df38b44d7`，前端 digest 为 `sha256:c16104f11122516aef809d01abd0074afcbcfe654a5a271dab62da5912aed89c`。
- 功能修复：Mihomo/Clash 普通导出器在无上游模板时改为生成 `PROXY` 选择组和真正的 `AUTO` url-test 组，不再把 `AUTO` 错误生成为 `select`；默认测速 URL 统一切换为 `https://cp.cloudflare.com/generate_204`，并通过迁移仅替换仍停留在旧 gstatic 默认值的设置和内置 Clash/Mihomo 模板。
- 验证追加：Mihomo 默认导出组结构单测通过；Mihomo 模板规则保留单测通过；Mihomo 内核 `-t` 配置校验通过；后端 Docker/Linux release 构建、前端生产构建和 diff whitespace 检查通过。
- 功能修复：内置 Clash/Mihomo 路由模板移除 `dns.fallback`、`fallback-filter` 和 `GEOIP,CN` 规则，避免 Mihomo 在缺少或无法下载 MMDB/GeoIP 数据库时整份配置初始化失败；SQLite 迁移和 MySQL 启动升级会清理旧内置模板中的同类默认片段，不覆盖用户自定义模板。
- 验证追加：使用用户提供的 `测试.yaml` 复现到 `can't initial GeoIP`，确认移除 DNS fallback/GeoIP 片段后 Mihomo `-t` 校验通过；新增内置 Clash 模板不含 GeoIP/MMDB 依赖的目标单测，并在 Docker/Linux builder 中通过。
- 功能修复：补全默认 Clash/Mihomo 导出规则；无模板 Clash 导出现在正确读取文档中的 ACL4SSR 风格完整模板，不再因中文标题匹配失败退回最小模板；内置 Clash/Mihomo 数据库模板升级为带 `rule-providers` 的常用分流规则，并在启动时仅升级仍停留在旧极简内容的内置模板。
- 验证追加：新的 Mihomo 内置模板和文档完整 Clash 模板均通过本地 Mihomo `-t` 结构校验；模板不重新引入 `GEOIP,CN`、`fallback-filter` 或 `geosite:cn` 依赖；后端目标单测、`cargo fmt` 和 diff whitespace 检查通过。
- 发布追加：使用 `DOCKER_REGISTRY=docker.m.daocloud.io` 重新编译并覆盖推送 Docker Hub `latest` 镜像；后端镜像 ID `31f52aa551e9`，digest `sha256:31f52aa551e9f681a1b5fecbc43679c721c33ed92b203ebd19c7770a34de185a`；前端镜像 ID `3753cdb6382d`，digest `sha256:3753cdb6382d41a2c6ee46a07f2df25d25f90a883600761e950e266b735e4daf`。
- 部署修复：针对前端日志 `backend could not be resolved`，后端镜像加入 `curl`，Compose 增加后端 `/healthz` 健康检查，并让前端等待后端健康后启动；Docker 文档补充排查命令，避免后端启动失败时前端先启动并持续返回 502。
- 发布追加：重新编译并推送后端 Docker Hub `latest` 镜像；后端镜像 ID `912e2d96e9e2`，大小 `158MB`，digest `sha256:912e2d96e9e251e2f41e11d5c99a452061da056d9262c3283489dfd1fd6acd68`。
- 部署修复：恢复历史迁移 `0006_latency_real_link_settings.sql` 的原始内容，避免已上线 SQLite 库启动时报 `Migrate(VersionMismatch(6))`；默认测速 URL 的变更保留在后续 `0012` 迁移中执行，确保历史迁移校验稳定。
- 发布追加：重新编译并推送后端 Docker Hub `latest` 镜像；后端镜像 ID `84e71d866a83`，大小 `158MB`，digest `sha256:84e71d866a8351b25a0fac780185532afa8eeead8adf128bc9f666a7838d2cc8`。
- 发布追加：为避免部署端 `latest` 被镜像代理或本地缓存卡住，额外推送不可变后端标签 `docker.io/jojhaa/sublinkx-rs-backend:20260623-migrate-fix`，digest 同为 `sha256:84e71d866a8351b25a0fac780185532afa8eeead8adf128bc9f666a7838d2cc8`。
- 发布准备：项目版本号更新到 `0.1.1`，补充 `CHANGELOG.md` 的 `v0.1.1` 发布说明，覆盖安全加固、Mihomo/Clash 导出、延迟测试、订阅二维码和 Docker 部署修复。
- 2026-07-08 功能修复：上游 Mihomo YAML 导入新增 `type: ss`/`shadowsocks` 支持，会转换为 SIP002 `ss://` 链接后走统一解析；Shadowsocks 解析器增强对 `userinfo` Base64、尾部 `/`、`udp` 和 `plugin` query 的兼容；Mihomo 再导出时保留 SS 的 `udp`/`plugin` 字段。
- 2026-07-08 功能增强：新增上游订阅链接管理能力，增加 `upstream_subscriptions` 持久化表、管理 API 和前端 `/upstreams` 页面；支持保存、编辑、删除、手动重新导入上游链接，并展示最近导入状态、导入/跳过/失败数量、目标节点分组和上游模板信息；旧节点页上游导入成功后也会自动写入管理表，管理页会从已有节点 `source_ref` 反向补齐历史上游链接。
- 2026-07-08 发布准备：项目版本号更新到 `0.1.2`，`CHANGELOG.md` 新增 `v0.1.2`，覆盖上游订阅链接管理页面和 Shadowsocks 上游导入兼容性修复。
- 2026-07-08 Docker 发布追加：使用 `DOCKER_REGISTRY=docker.m.daocloud.io` 完成 `0.1.2` 生产镜像构建并推送 Docker Hub；后端 `latest`/`0.1.2`/`v0.1.2` digest 为 `sha256:1e3f20b4dff0de8456a10a1beb173934f2251d36b920fdd960da51566bf2e52b`，前端 `latest`/`0.1.2`/`v0.1.2` digest 为 `sha256:2394535480207dc98cd9d4d795b80812dd325d4d1e433e893e560e832b0f1983`。
- 2026-07-08 前端优化追加：上游订阅管理页从复用固定列表格改为专用卡片列表，长订阅链接改为域名与短摘要展示，最近导入状态和操作按钮收敛到右侧信息区，避免长 URL 把页面撑乱或挤成竖排。
- 2026-07-08 功能调整追加：托管上游订阅取消手动选择节点分组，后端在创建、更新和重新导入时自动按上游订阅名称创建或复用节点分组；前端编辑弹窗改为展示自动分组说明，节点导入按订阅名隔离。
- 2026-07-08 Docker 发布追加：基于当前上游订阅自动分组和页面整理改动重新构建并推送 `0.1.2` 镜像；后端 `latest`/`0.1.2`/`v0.1.2` digest 为 `sha256:8a8224ea832fb588cd6b0488f0fe2545ead6c484b991fe1550fe896031486a24`，前端 `latest`/`0.1.2`/`v0.1.2` digest 为 `sha256:a3694fcb24d90566c18f0490c8b763299ffd6e2b01d26b730543383d7a926402`。
- 2026-07-09 缺陷修复：修复上游订阅删除后被节点 `source_ref` 自动回填、导致页面看起来无法删除的问题；删除上游订阅时保留已导入节点，但解除这些节点的上游来源关联。
- 2026-07-09 功能增强：删除上游订阅时新增“同时删除相关节点”可选行为；默认仍仅解除节点来源关联，选择删除节点时后端通过 `delete_nodes=true` 删除仍关联该上游链接的节点并返回删除数量。
- 2026-07-09 模板优化：Mihomo/Clash 默认模板的“节点选择”改为手动选择优先，再进入自动测速选择；无上游模板的 Mihomo 导出同步生成 `PROXY -> MANUAL -> AUTO` 结构，避免客户端默认落到自动选择；补充 AI、GitHub、YouTube、Netflix、Telegram、社媒流媒体和游戏平台等常用站点直连规则映射，并通过新迁移更新仍停留在内置模板内容的旧库记录。
- 2026-07-09 验证追加：`cargo fmt --manifest-path backend\Cargo.toml`、`cargo test --manifest-path backend\Cargo.toml`、`npm --prefix frontend run build`、`git diff --check` 均通过。
- 2026-07-09 发布准备：项目版本号更新到 `0.1.3`，`CHANGELOG.md` 新增 `v0.1.3`，覆盖上游订阅管理页面整理、自动分组、删除关联节点选项、Mihomo/Clash 手动优先模板和常用站点规则优化。
- 2026-07-09 Docker 发布追加：使用 `DOCKER_REGISTRY=docker.m.daocloud.io` 完成 `0.1.3` 生产镜像构建并推送 Docker Hub；后端 `latest`/`0.1.3`/`v0.1.3` digest 为 `sha256:5d32315ddb3c509707dd5c74a6c858be05ebb4f6a2ef0972fa3554111107390d`，前端 `latest`/`0.1.3`/`v0.1.3` digest 为 `sha256:dcd1c73a4cef8f6f113957173b6be275280490b7d484925cd397b48394810556`。
- 2026-07-09 Docker 发布验证：`docker buildx imagetools inspect docker.io/jojhaa/sublinkx-rs-backend:0.1.3` 和 `docker buildx imagetools inspect docker.io/jojhaa/sublinkx-rs-frontend:0.1.3` 均可读取远端镜像 digest；推送期间 Docker Desktop 内部 DNS 将 Docker Hub 解析到异常地址，已临时写入正确 hosts 映射完成推送并在完成后清理。
- 2026-07-09 GitHub 发布追加：提交 `81851c9 release v0.1.3` 已推送到 `main`，并创建/推送 tag `v0.1.3`；GitHub Release 已发布到 `https://github.com/jojhaa/sublinkx-rs/releases/tag/v0.1.3`，发布说明包含 `CHANGELOG.md` 的 `v0.1.3` 更新日志和 Docker 镜像 digest。
- 2026-07-17 功能增强：上游订阅新增定时同步能力，保存项可单独启用同步并设置 5 分钟到 7 天的同步间隔；后台任务会定期扫描到期上游订阅，按订阅名称自动分组执行覆盖同步。
- 2026-07-17 数据同步：上游同步改为覆盖当前来源节点，保留已有节点启用状态，优先更新同来源同指纹节点；若上游只是参数变化导致指纹变化但节点名称不变，会在名称唯一时按同来源同名节点原地覆盖；同步也会新增上游新增节点，并将上游已移除节点自动停用为 `upstream_missing`，后续上游恢复时自动重新启用；托管上游同步串行执行，若本次上游解析出现失败，会跳过 stale 停用以避免脏上游误停线上节点。
- 2026-07-17 管理页：上游订阅页面新增定时同步开关、同步间隔输入和同步状态徽标；最近一次同步统计持久化保存导入、更新、停用、跳过、失败数量，页面刷新后仍能看到真实覆盖结果。
- 2026-07-17 发布准备：项目版本号更新到 `0.1.4`，`CHANGELOG.md` 新增 `v0.1.4`，覆盖上游定时同步、同名参数变化原地覆盖、上游移除节点自动停用和同步统计更新。
- 2026-07-17 Docker 发布追加：使用 `DOCKER_REGISTRY=docker.m.daocloud.io` 完成 `0.1.4` 生产镜像构建并推送 Docker Hub；后端 `latest`/`0.1.4`/`v0.1.4` digest 为 `sha256:59344b70b6ff7f11ba69d9f0a64ed87809d4de71668e4a3e5bfe5284535bce19`，前端 `latest`/`0.1.4`/`v0.1.4` digest 为 `sha256:93dd1f832623e6789f433a4d787e062dd08f674150740b24fe524876adeb9642`。
- 2026-07-17 Docker 发布验证：`docker buildx imagetools inspect docker.io/jojhaa/sublinkx-rs-backend:0.1.4` 和 `docker buildx imagetools inspect docker.io/jojhaa/sublinkx-rs-frontend:0.1.4` 均可读取远端镜像 digest。
