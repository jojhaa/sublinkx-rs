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
