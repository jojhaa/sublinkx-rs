# SublinkX-RS

[English](README.en.md) | [更新日志](CHANGELOG.md) | [部署文档](docs/docker.md) | [最新版本](https://github.com/jojhaa/sublinkx-rs/releases/latest)

> 一个负责把订阅管好的后台，不负责把节点变快。速度交给线路，秩序交给它。

SublinkX-RS 是一个由个人开发者维护的自托管节点与订阅管理工具。它把上游同步、节点整理、模板编排、订阅分发、链路测试和出口 IP 探测放进同一个管理台，适合部署在自己的 Linux 服务器上长期运行。

后端使用 Rust、Axum、SQLx 和 Tokio，前端使用 Vue 3。开发者是个二次元，所以这里偶尔会出现一点轻松文案；但数据库迁移、鉴权和备份不会依靠友情、热血或突然觉醒。

SublinkX-RS 不是代理客户端，也不出售或生成节点。你需要使用自己合法持有的节点与上游订阅，毕竟 Rust 再快也不能无中生有。

当前稳定版本以 [GitHub Releases](https://github.com/jojhaa/sublinkx-rs/releases/latest) 为准。

## 先看它能做什么

- 把多个上游订阅和手动节点统一收进后台，不再维护“最终版2”和“这次真最终版”。
- 上游定时同步，参数变化原地更新，消失的节点先暂停而不是直接删除。
- 一个订阅按客户端自动返回 Mihomo、sing-box 或 v2rayN/Xray 格式，也能固定目标格式。
- 用真实 Mihomo 链路测延迟、成功率和抖动，不是端口能连上就播放胜利结算画面。
- 批量探测公网出口 IPv4/IPv6，并可接入外部国家情报服务生成地区策略组。
- 管理系统模板、自定义模板和上游原始模板，保留自己写分流与 DNS 的自由。

## 用 Docker 跑起来

下面是 Linux 生产服务器的最新版本部署。命令会自动解析 GitHub 最新 Release，再使用对应的源码标签和 Docker 镜像版本，不需要在 README 里手工维护版本号。

### 1. 获取部署配置

```bash
LATEST_URL="$(curl -fsSL -o /dev/null -w '%{url_effective}' \
  https://github.com/jojhaa/sublinkx-rs/releases/latest)"
LATEST_TAG="${LATEST_URL##*/}"
SUBLINKX_VERSION="${LATEST_TAG#v}"
test -n "$SUBLINKX_VERSION"

git clone --branch "$LATEST_TAG" --depth 1 https://github.com/jojhaa/sublinkx-rs.git
cd sublinkx-rs
cp .env.example .env

sed -i "s|^BACKEND_IMAGE=.*|BACKEND_IMAGE=docker.io/jojhaa/sublinkx-rs-backend:${SUBLINKX_VERSION}|" .env
sed -i "s|^FRONTEND_IMAGE=.*|FRONTEND_IMAGE=docker.io/jojhaa/sublinkx-rs-frontend:${SUBLINKX_VERSION}|" .env
printf '准备部署 SublinkX-RS %s\n' "$SUBLINKX_VERSION"
```

### 2. 修改 `.env`

设置 JWT 密钥和首次管理员凭据：

```env
JWT_SECRET=replace_with_a_random_secret_of_at_least_32_characters
BOOTSTRAP_ADMIN_USERNAME=replace_with_your_admin_username
BOOTSTRAP_ADMIN_PASSWORD=replace_with_a_strong_initial_password
```

生成随机 JWT 密钥：

```bash
openssl rand -base64 48
```

不要使用示例值、默认账号或短密码。互联网发现弱密码的速度，通常比你泡好一杯茶更快。

### 3. 校验并启动

```bash
docker compose config --quiet
docker compose pull
docker compose up -d
docker compose ps
curl -fsS http://127.0.0.1:3000/healthz
```

默认访问 `http://服务器IP:3000`。生产环境建议启用 HTTPS，外层 Nginx、Caddy 或面板反向代理统一转发到 `127.0.0.1:3000`，不要直接转发后端 `8080`。

更完整的 SQLite、PostgreSQL、MySQL、网段、反向代理和数据目录配置见 [Docker 部署文档](docs/docker.md)。

## 功能地图

| 模块 | 主要能力 |
| --- | --- |
| 节点管理 | 多行导入、Base64 解码、分组、状态筛选、批量操作和延迟状态 |
| 上游订阅 | 手动与定时同步、同名参数覆盖、缺失节点暂停、按上游名称分组 |
| 订阅管理 | 固定节点、跟随分组、到期与续期、二维码、Token 轮换和客户端识别 |
| 模板管理 | 系统模板、自定义模板、上游 Mihomo 模板和批量管理 |
| 链路测试 | 1、3、5 轮实时测试，统计最低、平均、最高延迟、抖动和成功率 |
| 出口探测 | 批量获取公网 IPv4/IPv6，支持定时执行和上游同步后自动排队 |
| 系统设置 | Mihomo 内核、并发、缓存、限流、探测周期和国家策略门槛 |

## 支持范围

### 上游导入

Mihomo/Clash YAML 当前覆盖以下协议：

| 协议 | 导入 |
| --- | :---: |
| Shadowsocks | 支持 |
| VMess / VLESS | 支持 |
| Trojan | 支持 |
| Hysteria2 | 支持 |
| TUIC | 支持 |
| WireGuard | 支持 |
| AnyTLS | 支持 |

同时兼容 Clash Provider `payload`、SIP008 Shadowsocks JSON、UTF-8 BOM 和双层 Base64。未知协议或字段不完整的节点会进入失败明细，不会假装一切正常。

### 订阅输出

| 客户端家族 | 输出方式 |
| --- | --- |
| Mihomo / Clash | YAML 配置与模板编排 |
| sing-box | JSON 配置 |
| v2rayN / Xray | 多协议 URI Bundle |
| Surge | Surge 配置 |
| Quantumult X | 完整配置、风险策略组与本地规则 |
| Shadowrocket | 兼容 URI Bundle 或独立完整配置 |
| 其他客户端 | 通过内置或自定义模板输出 |

完整支持情况见 [客户端兼容矩阵](docs/client-compatibility.md) 和 [协议与客户端矩阵](docs/protocol-client-matrix.md)。客户端如果不肯好好报告 User-Agent，可以在订阅链接中固定 `target`，不和它猜谜。

## Mihomo 在这里做什么

Mihomo 不只是被下载后放在目录里当吉祥物，它负责：

- 节点真实链路延迟测试。
- 多轮链路测试和实时结果输出。
- 节点公网出口 IP 探测。
- 手动选择、最低延迟、故障转移和负载策略组。
- 按已验证国家信息生成地区策略组。

后台测速、手动测速、链路测试和 IP 探测共用任务锁，避免一激动拉起一排 Mihomo 进程把服务器围住。管理员可以在“系统设置”中检测、下载或指定内核。

出口 IP 探测只访问固定的 [ipify](https://www.ipify.org/) 双栈接口，不接受任意测试 URL。目标服务只能看到节点出口 IP，不会收到节点链接、协议密码、订阅正文或管理员凭据。

## 数据与升级

默认使用 SQLite，数据保存在：

```text
docker-data/
  backend/
    app.db
  mihomo/
    mihomo
```

也可以切换到 PostgreSQL 14+ 或 MySQL 8.x，具体配置见 [Docker 部署文档](docs/docker.md)。应用启动时不会擅自搬库；需要切换时可使用离线 `migrate-database check/run` 工具，在备份、停服务和空目标库的前提下迁移完整业务数据。

升级前请备份 `.env`、数据库和数据挂载目录。正式挑战迁移 Boss 之前，先留一个能读档的存档点。

```bash
docker compose config --quiet
docker compose pull
docker compose up -d
docker compose ps
```

不要使用 `docker compose down -v` 更新服务。这个 `-v` 很短，删除数据卷后的故事可能很长。

## 额外项目：国家情报服务

SublinkX-RS 本体只负责获取真实出口 IP，不内置 GeoIP、ASN 或风险数据库。

`ip-intelligence-rs` 是另一个独立项目，不包含在 SublinkX-RS 仓库、Docker 镜像或默认 Compose 中，也不会随着 SublinkX-RS 自动安装。只有需要国家、ASN 或风险情报时，才需要另外获取、部署和维护它。

额外项目部署完成后，可以通过以下环境变量与 SublinkX-RS 对接：

```env
IP_INTELLIGENCE_ENABLED=true
IP_INTELLIGENCE_BASE_URL=http://ip-intelligence-api:8090/api/v1
IP_INTELLIGENCE_API_TOKEN=replace_with_the_shared_api_token
IP_INTELLIGENCE_SOURCE_KEY=replace_with_a_unique_source_key
```

两个独立项目位于同一 Docker 网络时，应给情报 API 设置唯一别名，例如 `ip-intelligence-api`。不要复用 `backend`，不同 Compose 项目的同名服务会让 Docker DNS 开始左右为难。

情报服务不可用不会覆盖已取得的出口 IP，也不会接收到代理节点认证信息。它负责认 IP，不负责翻你的订阅家底。

已登录管理员可以按节点批量读取 Scamalytics 风险分：

```http
POST /api/v1/node-ip-probes/intelligence/risk-scores
Authorization: Bearer <access_token>
Content-Type: application/json

{"ids":[12,18,27]}
```

单次最多查询 200 个节点。后端使用节点最近一次成功探测的公网出口 IP，同一 IP 只请求情报服务一次，外部查询并发限制为 8。响应为每个节点返回 `scamalytics_fraud_score`、`scamalytics_isp_risk_score`、情报缓存状态以及 `complete`、`partial`、`pending` 或 `failed` 结果状态。

情报服务的受保护查询接口只立即返回本地缓存；首次查询或缓存过期时会在情报服务内排队补全，因此本次结果可能为 `pending`，稍后再次查询即可取得更新后的分数。该接口不会启动 Mihomo，也不会修改节点、订阅或探测记录。

## 安全边界

- 登录密码使用 Argon2 哈希，生产环境必须配置独立 JWT 密钥和首次管理员凭据。
- Web 登录使用 HttpOnly Cookie、CSRF 校验和服务端安全退出。
- 上游下载限制目标地址、响应体和节点数量，阻止内网地址及 DNS rebinding 类 SSRF。
- 公开订阅提供缓存、单 IP/全局限流、日志脱敏和禁止搜索引擎索引。
- JWT、数据库连接、Cookie、可信代理和情报服务 Token 只通过服务端环境变量配置。

公开反馈问题时，请先删除订阅 URL、Token、密码、私钥、节点地址和生产日志中的个人信息。Bug 可以公开，家底不用一起公开。

## 本地开发

需要 Rust stable、Node.js 20+ 和 npm。

Windows 使用 PowerShell 7：

```powershell
.\scripts\dev.ps1
```

Linux 或 macOS：

```bash
chmod +x scripts/dev.sh
./scripts/dev.sh
```

默认开发地址：

```text
Backend  http://127.0.0.1:8080
Frontend http://127.0.0.1:5173
SQLite   backend/data/app.db
```

## 遇到问题

提交 [Issue](https://github.com/jojhaa/sublinkx-rs/issues) 时，建议附上：

- SublinkX-RS 版本和部署方式。
- 操作系统、CPU 架构和数据库类型。
- 可以稳定复现的步骤。
- 已脱敏的错误日志或截图。
- 预期结果与实际结果。

这是个人维护项目，回复速度偶尔会受到现实副本和 Bug Boss 的影响，但能复现的问题都会认真看。

## 关于项目和开发者

开发者喜欢 Rust、Vue，也喜欢二次元。目标不是把控制台做成魔法阵，而是让复杂的订阅管理少一点重复劳动，多一点可控和安心。

项目从 [gooaclok819/sublinkX](https://github.com/gooaclok819/sublinkX) 的订阅分发思路出发，后来逐步重构成 Rust 后端和 Vue 3 前端。当前仓库不是原项目官方仓库，感谢原作者和相关开源项目维护者打下的基础。

## 文档

- [更新日志](CHANGELOG.md)
- [Docker 部署](docs/docker.md)
- [客户端兼容矩阵](docs/client-compatibility.md)
- [协议与客户端矩阵](docs/protocol-client-matrix.md)
- [客户端目标注册表](docs/client-target-registry.md)
- [Clash/Mihomo 分流模板说明](docs/clash-routing-template.md)

## 许可证

SublinkX-RS 使用 [AGPL-3.0-or-later](LICENSE)。修改、分发或以网络服务方式提供衍生版本时，需要按许可证要求提供对应源代码。

原项目 `gooaclok819/sublinkX` 使用 MIT License。分发或修改本项目时，请继续保留原项目和本项目要求的版权与许可证声明，也不要暗示原项目作者为本项目提供官方认可或背书。
