# 客户端兼容矩阵

更新日期：2026-05-29

本文档用于规划 `sublinkx-rs` 需要优先适配的主流代理客户端。新系统不应只保留旧版 `clash / surge / v2ray` 输出，而应该按“客户端家族”设计导出器和兼容策略。

## 1. 兼容策略

订阅导出不应该被当成单一功能处理。后端需要支持多个输出家族：

- Mihomo / Clash 家族配置
- Xray / v2rayN 家族 URI 订阅
- sing-box outbound 配置
- Surge 家族配置

后端架构需要具备：

- 多导出渲染器
- 按协议能力过滤节点
- 按客户端生成配置
- 按客户端家族维护格式版本

## 2. 优先级

### Tier 1：MVP 或早期必须支持

这些是第一优先级客户端家族。

1. Mihomo / Clash 家族
   - Clash Verge Rev
   - Stash
   - 其它 Mihomo 兼容 GUI

2. v2rayN / Xray 家族
   - v2rayN
   - v2rayNG

3. Surge 家族
   - Surge
   - Shadowrocket 可在部分场景通过 Surge 风格输出兼容

4. sing-box 家族
   - NekoBox for Android
   - Hiddify App
   - 其它 sing-box GUI 客户端

### Tier 2：核心稳定后继续适配

- Shadowrocket 专用输出
- 支持 rule-set 的 sing-box profile
- GUI.for.SingBox 兼容验证
- Hiddify 专用配置优化

## 3. 当前主流客户端信号

以下信息检查于 2026-05-29：

- Clash Verge Rev 仍在维护，定位为基于 Mihomo 的现代跨平台 GUI。
- v2rayN 仍在维护，支持 Windows、Linux、macOS，并支持 Xray、sing-box 等内核。
- v2rayNG 仍在维护，是 Android 平台常用的 Xray / v2fly 客户端。
- NekoBox for Android 支持 ClashMeta、v2rayN、sing-box outbound 等常见订阅格式。
- Hiddify App 是基于 sing-box 的多平台客户端。
- Stash 是 Apple 平台上常见的 Clash 兼容客户端。
- Shadowrocket 仍是 Apple 平台广泛使用的规则代理客户端。

## 4. 导出器目标

后端应提供独立导出器，而不是把所有格式混在一个输出路径里：

- `mihomo`
- `surge`
- `xray_uri_bundle`
- `sing_box_outbound_bundle`

推荐公开访问形式：

- `/s/{token}?target=mihomo`
- `/s/{token}?target=surge`
- `/s/{token}?target=xray`
- `/s/{token}?target=sing-box`

后续可增加客户端别名：

- `client=clash-verge-rev`
- `client=v2rayn`
- `client=v2rayng`
- `client=nekobox`
- `client=hiddify`
- `client=stash`
- `client=shadowrocket`

后端内部应把客户端别名映射到导出器家族和客户端专用转换规则。

## 5. 兼容矩阵设计

每个协议处理器都应声明自己支持哪些导出目标：

- 是否可导出到 Mihomo
- 是否可导出到 Surge
- 是否可导出到 Xray URI bundle
- 是否可导出到 sing-box outbound

这样可以避免把不兼容节点静默导出给错误客户端。

示例：

- `wireguard` 可能支持 sing-box，但不一定支持所有 Surge 版本。
- `reality` transport 可用于 sing-box 和 Xray 家族，但需要客户端专用字段。
- 某些高级插件可能只适合 Mihomo 兼容输出。

## 6. 产品规则

导出行为规则：

1. 不要把不支持的节点静默写入目标客户端格式。
2. 管理后台预览必须展示明确兼容警告。
3. 每个订阅可以设置默认客户端目标。
4. 每个客户端可以设置过滤规则。
5. 同时支持 `strict` 和 `best_effort`。

`strict`：

- 只要选中节点中有任意节点不兼容目标客户端，就导出失败。

`best_effort`：

- 自动移除不兼容节点，并记录警告。

## 7. 前端要求

管理后台需要展示：

- 目标客户端家族选择器
- 每个节点的兼容性 badge
- 不兼容节点列表
- 导出预览
- 每个订阅的默认客户端目标

## 8. 实现顺序

推荐实现顺序：

1. Mihomo 导出器
2. Xray URI bundle 导出器，面向 v2rayN / v2rayNG
3. Surge 导出器
4. sing-box outbound 导出器
5. 客户端专用兼容优化

## 9. 参考来源

检查日期：2026-05-29

- [Clash Verge Rev](https://github.com/clash-verge-rev/clash-verge-rev)
- [v2rayN](https://github.com/2dust/v2rayN)
- [v2rayNG](https://github.com/2dust/v2rayNG)
- [NekoBox for Android](https://github.com/MatsuriDayo/NekoBoxForAndroid)
- [Hiddify App](https://github.com/hiddify/hiddify-app)
- [Stash Docs](https://stash.wiki/)
- [Stash App Store page](https://apps.apple.com/us/app/stash/id1596063349?l=zh-CN&l=zh-Hans-CN%3Fplatform%3Diphone)
- [Shadowrocket App Store page](https://apps.apple.com/us/app/shadowrocket/id932747118?l=en-us)

相关文档：

- [协议 x 客户端矩阵](protocol-client-matrix.md)
