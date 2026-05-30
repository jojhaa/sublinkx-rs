# 协议 x 客户端矩阵

更新日期：2026-05-29

这个矩阵用于规划 `sublinkx-rs` 的自动导出能力，不代表客户端手写配置可以支持的所有边界情况。

我们关心的问题是：

- 如果后端生成订阅或 profile，某个客户端家族是否应该被视为该协议的一等导出目标？

## 图例

- `✅` 有明确文档依据，适合作为一等导出目标
- `⚠️` 支持但有明显限制，例如版本差异、传输限制或字段差异
- `◐` 从官方发布记录、商店信息或产品说明可观察到支持，但仍需真机验证
- `❌` 没有足够官方依据，或不适合作为 v1 自动导出目标

## 客户端家族

- **Mihomo**：Mihomo core / Clash Verge Rev 类客户端
- **Stash**：Apple 平台的 Stash
- **Surge**：Apple 平台的 Surge
- **Xray**：v2rayN / v2rayNG 类客户端
- **sing-box**：NekoBox / Hiddify / 其它 sing-box GUI
- **Shadowrocket**：Apple 平台的 Shadowrocket

## 矩阵

| 协议 / 能力 | Mihomo | Stash | Surge | Xray | sing-box | Shadowrocket | 说明 |
|---|---|---:|---:|---:|---:|---:|---|
| `Shadowsocks` | ✅ | ✅ | ✅ | ✅ | ✅ | ◐ | 各客户端家族的通用基础协议。 |
| `Shadowsocks 2022` | ✅ | ✅ | ❌ | ⚠️ | ✅ | ◐ | 应和传统 SS 分开建模；Surge 文档未列为原生类型。 |
| `ShadowsocksR` | ✅ | ✅ | ❌ | ❌ | ❌ | ◐ | 仅作为兼容导入保留，不建议作为长期主推协议。 |
| `SOCKS5` | ✅ | ✅ | ✅ | ✅ | ✅ | ⚠️ | 兼容性好，但不一定作为优先订阅导出目标。 |
| `HTTP / HTTPS proxy` | ✅ | ✅ | ✅ | ❌ | ✅ | ⚠️ | Xray 家族订阅路径不应作为一等目标。 |
| `VMess` | ✅ | ✅ | ✅ | ✅ | ✅ | ◐ | 仍广泛兼容，但不是长期最推荐的新协议。 |
| `VLESS` | ✅ | ✅ | ❌ | ✅ | ✅ | ◐ | Surge 文档未列为原生代理类型。 |
| `Reality` transport | ⚠️ | ⚠️ | ❌ | ✅ | ⚠️ | ◐ | 这是传输/安全能力，不是独立协议。 |
| `Trojan` | ✅ | ✅ | ✅ | ✅ | ✅ | ◐ | 主流覆盖较好。 |
| `Hysteria 1` | ✅ | ✅ | ❌ | ❌ | ✅ | ◐ | 旧版现代协议桥接，部分客户端不再强调。 |
| `Hysteria 2` | ⚠️ | ✅ | ✅ | ⚠️ | ✅ | ◐ | Xray 家族支持不均匀。 |
| `TUIC` | ✅ | ✅ | ✅ | ⚠️ | ✅ | ◐ | Xray 家族兼容性不如 Mihomo/sing-box 稳定。 |
| `WireGuard` | ✅ | ✅ | ✅ | ⚠️ | ✅ | ◐ | 整体较强，但各客户端导出字段差异大。 |
| `Snell` | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ | 只在明确支持的客户端中作为目标。 |
| `AnyTLS` | ⚠️ | ✅ | ✅ | ✅ | ✅ | ◐ | Mihomo 支持 AnyTLS，但明确不支持 `AnyTLS + Reality`。 |
| `NaiveProxy` | ❌ | ❌ | ❌ | ❌ | ✅ | ◐ | sing-box 优先能力；Shadowrocket 有发布记录依据。 |
| `SSH` | ❌ | ✅ | ✅ | ❌ | ✅ | ❌ | v1 不优先支持 Mihomo/Xray 导出。 |
| `ShadowTLS` | ⚠️ | ⚠️ | ⚠️ | ❌ | ✅ | ❌ | 通常作为混淆层/插件建模，不应简单当独立节点协议。 |
| `Juicity` | ❌ | ✅ | ❌ | ❌ | ❌ | ❌ | 当前矩阵中主要是 Stash 一等目标。 |

## 解释规则

### 1. 传输能力需要单独建模

以下能力不应像 `ss` 或 `trojan` 一样直接作为顶层协议：

- `Reality`
- `XTLS / Vision`
- `WS`
- `gRPC`
- `HTTP/2`
- `HTTP/3`
- `ShadowTLS`
- `ECH`

推荐拆分为：

- 协议
- 传输
- TLS 能力
- 可选混淆或扩展层

### 1.1 VLESS Reality 和 AnyTLS 处理

`VLESS + Reality` 应建模为 `protocol = vless` 且 `security = reality`，并保存 `pbk`、`sid`、`sni`、`fp`、`flow` 等 Reality 字段。

`AnyTLS` 在本系统中作为独立协议目标处理。不要把 `AnyTLS + Reality` 作为 Mihomo 支持组合；Mihomo 官方文档明确说明该组合不支持且不会支持。Reality 场景优先使用 VLESS、VMess 或 Trojan 的传输字段建模。

### 2. 优先实现的导出器家族

根据矩阵，第一批导出目标应为：

1. `mihomo`
2. `xray_uri_bundle`
3. `surge`
4. `sing_box_outbound_bundle`

客户端别名映射：

- `clash-verge-rev` -> `mihomo`
- `stash` -> `mihomo`，后续增加 Stash 专用转换
- `v2rayn` -> `xray_uri_bundle`
- `v2rayng` -> `xray_uri_bundle`
- `surge` -> `surge`
- `nekobox` -> `sing_box_outbound_bundle`
- `hiddify` -> `sing_box_outbound_bundle`
- `shadowrocket` -> 先走 `surge` 或 `apple_compat` 桥接路径，后续再做专用优化

### 3. Strict 与 best-effort 导出

每个目标家族都应支持两种导出模式：

- `strict`
- `best_effort`

Strict：

- 只要有选中节点不兼容目标客户端，导出失败。

Best effort：

- 丢弃不兼容节点。
- 在预览和日志中记录警告。

## 推荐 V1 协议集合

第一阶段最适合覆盖主流客户端的协议：

- `Shadowsocks`
- `VMess`
- `VLESS`
- `Trojan`
- `Hysteria2`
- `TUIC`
- `WireGuard`

仅兼容保留：

- `ShadowsocksR`
- `Hysteria 1`
- `Snell`

后续专项支持：

- `AnyTLS`
- `NaiveProxy`
- `SSH`
- `Juicity`
- `ShadowTLS`

## 对后端设计的影响

Rust 后端需要建模：

- 协议身份
- 传输身份
- TLS 能力
- UDP 能力
- 导出器兼容性
- 客户端家族覆盖规则

示例：

```rust
pub struct ProtocolCapabilities {
    pub mihomo: SupportLevel,
    pub stash: SupportLevel,
    pub surge: SupportLevel,
    pub xray_family: SupportLevel,
    pub sing_box_family: SupportLevel,
    pub shadowrocket: SupportLevel,
}
```

## 对前端设计的影响

Vue 3 管理后台需要展示：

- 目标客户端家族选择器
- 节点兼容性 badge
- 不兼容节点警告
- strict / best-effort 切换
- 每个订阅的默认导出目标

## 参考来源

本矩阵基于 2026-05-29 检查的官方来源：

- [Mihomo docs](https://wiki.metacubex.one/en/config/)
- [Stash protocol types](https://stash.wiki/en/proxy-protocols/proxy-types)
- [Surge proxy policy](https://manual.nssurge.com/policy/proxy.html)
- [v2rayN subscription description](https://github.com/2dust/v2rayN/wiki/Description-of-subscription)
- [v2rayNG repository](https://github.com/2dust/v2rayNG)
- [sing-box outbound types](https://sing-box.sagernet.org/configuration/outbound/)
- [NekoBox for Android](https://github.com/MatsuriDayo/NekoBoxForAndroid)
- [Hiddify App](https://github.com/hiddify/hiddify-app)
- [Shadowrocket App Store page](https://apps.apple.com/us/app/shadowrocket/id932747118?l=en-us)

标记为 `◐` 的单元格主要来自官方应用商店更新记录或产品说明，仍建议真机验证。
