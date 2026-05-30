# 客户端目标注册表与模板预设

更新日期：2026-05-29

本文档把 `sub-web-modify` 中的客户端列表整理为 `sublinkx-rs` 原生架构规划。

`sub-web-modify` 主要是 subconverter 前端，会暴露大量 target 和远程配置预设，并生成类似下面的 URL：

```text
/sub?target=clash&url=...&config=...&emoji=true&udp=true
```

`sublinkx-rs` 不直接复制这种 URL 形态。Rust 后端应自己管理客户端注册表、导出器选择、兼容规则和模板/预设目录。

## 1. 目标注册表

每个公开 target 应注册一次，并包含：

- 稳定 key
- 展示名称
- 别名
- User-Agent 识别标记
- 导出器家族
- 模板类型
- 实现状态

导出器家族应少于客户端 target 数量：

| 导出器家族 | 用途 |
|---|---|
| `xray` | Xray、V2Ray、v2rayN、v2rayNG 的 URI bundle，也可作为临时 Shadowrocket 桥接输出 |
| `mihomo` | Mihomo、Clash、Clash Verge Rev、Stash 兼容路径的 YAML profile |
| `surge` | Surge 风格 INI profile |
| `sing-box` | sing-box JSON profile |
| `not_implemented` | 已知 target，但暂时没有安全的原生导出器 |

当前后端注册表文件：

- `backend/src/domain/client.rs`

## 2. sub-web-modify target 映射

| sub-web-modify 标签 | sub-web target | sublinkx-rs target | 当前状态 |
|---|---:|---:|---|
| Clash | `clash` | `clash` | 已通过 Mihomo 导出器和 Clash 模板类型实现 |
| Surge4/5 | `surge&ver=4` | `surge` | 已实现 |
| Sing-Box | `singbox` | `sing-box` | 已实现 |
| V2Ray | `v2ray` | `xray` | 已作为 URI bundle 实现 |
| ShadowRocket | `shadowrocket` | `shadowrocket` | 先作为 Xray URI 桥接实现 |
| Surge3 | `surge&ver=3` | `surge3` | 规划中 |
| Surge2 | `surge&ver=2` | `surge2` | 规划中 |
| Quantumult X | `quanx` | `quanx` | 规划中 |
| Quantumult | `quan` | `quan` | 规划中 |
| Loon | `loon` | `loon` | 规划中 |
| Surfboard | `surfboard` | `surfboard` | 规划中 |
| Mellow | `mellow` | `mellow` | 规划中 |
| ClashR | `clashr` | `clashr` | 规划中 |
| Shadowsocks SIP002 | `ss` | `ss` | 规划中 |
| Shadowsocks Android SIP008 | `sssub` | `sssub` | 规划中 |
| ShadowsocksR | `ssr` | `ssr` | 规划中 |
| ShadowsocksD | `ssd` | `ssd` | 规划中 |
| Trojan | `trojan` | `trojan` | 规划中 |
| Mixed | `mixed` | `mixed` | 规划中 |
| Auto | `auto` | 自动识别 `/s/{token}` | 部分实现 |

## 3. 模板类型策略

当客户端输出形态不同，模板 `kind` 应和客户端 target 对齐。即使某些导出器尚未完全实现，后端也可以先识别对应模板类型，方便后续补齐。

已实现模板类型：

- `common`
- `clash`
- `mihomo`
- `xray`
- `surge`
- `sing-box`

规划中的模板类型：

- `surge2`
- `surge3`
- `quanx`
- `quan`
- `loon`
- `surfboard`
- `mellow`
- `clashr`
- `ss`
- `sssub`
- `ssr`
- `ssd`
- `trojan`
- `mixed`

## 4. 远程预设目录

`sub-web-modify` 提供很多远程配置，尤其是 ACL4SSR 和社区规则预设。在 `sublinkx-rs` 中，这些内容应成为独立的 preset catalog，而不是混在订阅记录里。

推荐表结构：

```sql
CREATE TABLE template_presets (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    target_kind TEXT NOT NULL,
    source_type TEXT NOT NULL CHECK (source_type IN ('built_in', 'remote_url', 'database')),
    source_url TEXT,
    content TEXT,
    description TEXT NOT NULL DEFAULT '',
    enabled INTEGER NOT NULL DEFAULT 1,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

预设规则：

- 内置预设随后端发布，默认可信。
- 远程 URL 预设必须使用 HTTPS，并设置大小限制和超时限制。
- 订阅可以绑定具体模板，也可以绑定预设。
- 如果需要可复现渲染，渲染前应复制或快照预设内容。

## 5. 导出参数模型

`sub-web-modify` 暴露很多 flags，例如：

- `emoji`
- `udp`
- `xudp`
- `tfo`
- `sort`
- `expand`
- `scv`
- `fdn`
- `appendType`
- `clash.doh`
- `surge.doh`
- `singbox.ipv6`

在本项目中，这些参数应存成结构化导出选项：

```rust
pub struct ExportOptions {
    pub emoji: bool,
    pub udp: bool,
    pub xudp: bool,
    pub tcp_fast_open: bool,
    pub sort_nodes: bool,
    pub expand_rules: bool,
    pub skip_cert_verify: bool,
    pub filter_deprecated_nodes: bool,
    pub append_protocol_type: bool,
    pub doh: bool,
    pub ipv6: bool,
}
```

早期后端可以先保持查询参数简单，等导出器注册表稳定后再把这些选项绑定到订阅级配置。

## 6. 实现顺序

推荐顺序：

1. 稳定当前已实现目标：`xray`、`clash`、`mihomo`、`surge`、`sing-box`。
2. 增加 preset catalog API 和内置 ACL4SSR 风格 Clash 预设管理。
3. 增加 Shadowrocket 专用导出器，替代 Xray 桥接输出。
4. 增加 Quantumult X 和 Loon 导出器。
5. 增加 Surfboard 和 Mellow 导出器。
6. 增加原始协议 target：`ss`、`sssub`、`ssr`、`ssd`、`trojan`。
7. 增加 mixed export。
8. 如真实用户仍需要，再补 Surge 2/3 兼容分支。

## 7. 设计规则

- 已识别 target 仍可能是 `planned`，API 必须明确返回状态。
- 不要假装规划中 target 已完整支持。
- User-Agent 自动识别只能选择已实现 target。
- 模板校验可以接受规划中的模板类型，方便运维提前准备模板。
- 导出器兼容性必须协议感知、模式感知：`strict` 失败，`best_effort` 丢弃不兼容节点。
