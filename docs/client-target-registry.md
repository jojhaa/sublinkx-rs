# Client Target Registry And Template Presets

Updated: 2026-05-29

This document turns the `sub-web-modify` client list into a native
`sublinkx-rs` architecture plan.

`sub-web-modify` is mainly a subconverter frontend. It exposes many client
targets and remote config presets, then builds a URL such as:

```text
/sub?target=clash&url=...&config=...&emoji=true&udp=true
```

For this rewrite, we should not copy that shape directly. The Rust backend
should own the client registry, renderer selection, compatibility rules, and
template/preset catalog.

## 1. Target Registry

Every public target should be registered once with:

- stable key
- display label
- aliases
- user-agent markers
- renderer family
- template kind
- implementation status

Renderer families are deliberately smaller than client targets:

| Renderer family | Purpose |
|---|---|
| `xray` | URI bundle for Xray, V2Ray, v2rayN, v2rayNG, and temporary Shadowrocket bridge output |
| `mihomo` | YAML profile for Mihomo, Clash, Clash Verge Rev, Stash-compatible paths |
| `surge` | Surge-style INI profile |
| `sing-box` | sing-box JSON profile |
| `not_implemented` | Known target, but no safe native renderer yet |

Current backend registry file:

- `backend/src/domain/client.rs`

## 2. sub-web-modify Target Mapping

| sub-web-modify label | sub-web target | sublinkx-rs target | Current status |
|---|---:|---:|---|
| Clash | `clash` | `clash` | Implemented through Mihomo renderer with Clash template kind |
| Surge4/5 | `surge&ver=4` | `surge` | Implemented |
| Sing-Box | `singbox` | `sing-box` | Implemented |
| V2Ray | `v2ray` | `xray` | Implemented as URI bundle |
| ShadowRocket | `shadowrocket` | `shadowrocket` | Implemented as Xray URI bridge first |
| Surge3 | `surge&ver=3` | `surge3` | Planned |
| Surge2 | `surge&ver=2` | `surge2` | Planned |
| Quantumult X | `quanx` | `quanx` | Planned |
| Quantumult | `quan` | `quan` | Planned |
| Loon | `loon` | `loon` | Planned |
| Surfboard | `surfboard` | `surfboard` | Planned |
| Mellow | `mellow` | `mellow` | Planned |
| ClashR | `clashr` | `clashr` | Planned |
| Shadowsocks SIP002 | `ss` | `ss` | Planned |
| Shadowsocks Android SIP008 | `sssub` | `sssub` | Planned |
| ShadowsocksR | `ssr` | `ssr` | Planned |
| ShadowsocksD | `ssd` | `ssd` | Planned |
| Trojan | `trojan` | `trojan` | Planned |
| Mixed | `mixed` | `mixed` | Planned |
| Auto | `auto` | automatic `/s/{token}` detection | Partially implemented |

## 3. Template Kind Strategy

Template `kind` should match the client target when the output shape differs.
The backend now recognizes all registered template kinds even before every
renderer exists.

Implemented template kinds:

- `common`
- `clash`
- `mihomo`
- `xray`
- `surge`
- `sing-box`

Planned template kinds:

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

## 4. Remote Preset Catalog

`sub-web-modify` offers many remote configs, especially ACL4SSR and community
rule presets. In `sublinkx-rs`, this should become a separate preset catalog
instead of being mixed into subscription records.

Recommended schema:

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

Preset rules:

- Built-in presets are shipped with the backend and safe by default.
- Remote URL presets must use HTTPS, size limits, and timeout limits.
- A subscription may bind either a concrete template or a preset.
- Presets should be copied or snapshotted before rendering if reproducibility is required.

## 5. Export Parameter Model

sub-web-modify exposes flags such as:

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

In this rewrite, these should be stored as structured export options:

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

The initial backend can keep query parameters minimal, then add these options
per subscription after the renderer registry is stable.

## 6. Delivery Order

Recommended order:

1. Keep current targets stable: `xray`, `clash`, `mihomo`, `surge`, `sing-box`.
2. Add preset catalog API and built-in ACL4SSR-style Clash preset management.
3. Add Shadowrocket dedicated renderer instead of Xray bridge output.
4. Add Quantumult X and Loon renderers.
5. Add Surfboard and Mellow renderers.
6. Add raw protocol targets: `ss`, `sssub`, `ssr`, `ssd`, `trojan`.
7. Add mixed export.
8. Add Surge 2/3 compatibility splits if still needed by real users.

## 7. Design Rules

- A recognized target may still be `planned`; the API must say so clearly.
- Do not silently pretend planned targets are fully supported.
- User-agent auto detection should only choose implemented targets.
- Template validation may accept planned template kinds so operators can prepare templates early.
- Renderer compatibility remains protocol-aware and mode-aware: `strict` fails, `best_effort` drops unsupported nodes.
