# Protocol x Client Family Matrix

Updated: 2026-05-29

This matrix is for **`sublinkx-rs` export planning**, not for every possible hand-written custom profile a client might load manually.

The question we care about is:

- If our backend emits a generated subscription/profile, should we treat the client family as a first-class target for that protocol?

## Legend

- `✅` Documented and suitable as a first-class target
- `⚠️` Supported with important caveats, version splits, or transport restrictions
- `◐` Observed in official release notes or app metadata, but still requires real-device verification
- `❌` No official support evidence for our planned export path, or not worth targeting in v1

## Client Families

- **Mihomo**: Mihomo core / Clash Verge Rev class clients
- **Stash**: Stash on Apple platforms
- **Surge**: Surge on Apple platforms
- **Xray**: v2rayN / v2rayNG class clients
- **sing-box**: NekoBox / Hiddify / other sing-box GUI clients
- **Shadowrocket**: Shadowrocket on Apple platforms

## Matrix

| Protocol / Feature | Mihomo | Stash | Surge | Xray | sing-box | Shadowrocket | Notes |
|---|---|---:|---:|---:|---:|---:|---|
| `Shadowsocks` | ✅ | ✅ | ✅ | ✅ | ✅ | ◐ | Common baseline across all families. |
| `Shadowsocks 2022` | ✅ | ✅ | ❌ | ⚠️ | ✅ | ◐ | Treat as separate capability from classic SS. Surge docs do not list it as a native type. |
| `ShadowsocksR` | ✅ | ✅ | ❌ | ❌ | ❌ | ◐ | Keep only for compatibility imports, not as a preferred long-term protocol. |
| `SOCKS5` | ✅ | ✅ | ✅ | ✅ | ✅ | ⚠️ | Good compatibility, but not always a preferred subscription export target. |
| `HTTP / HTTPS proxy` | ✅ | ✅ | ✅ | ❌ | ✅ | ⚠️ | Xray-family subscription path is not a first-class target here. |
| `VMess` | ✅ | ✅ | ✅ | ✅ | ✅ | ◐ | Still widely compatible, but not the long-term preferred modern choice. |
| `VLESS` | ✅ | ✅ | ❌ | ✅ | ✅ | ◐ | Surge docs do not list VLESS as a native proxy type. |
| `Reality` transport | ⚠️ | ⚠️ | ❌ | ✅ | ⚠️ | ◐ | This is a transport/capability, not a standalone protocol. Must be modeled separately. |
| `Trojan` | ✅ | ✅ | ✅ | ✅ | ✅ | ◐ | Strong mainstream coverage. |
| `Hysteria 1` | ✅ | ✅ | ❌ | ❌ | ✅ | ◐ | Treat as legacy-modern bridge; not all families still emphasize it. |
| `Hysteria 2` | ⚠️ | ✅ | ✅ | ⚠️ | ✅ | ◐ | Xray-family support is uneven across v2rayN/v2rayNG paths. |
| `TUIC` | ✅ | ✅ | ✅ | ⚠️ | ✅ | ◐ | Xray-family support is not as uniform as Mihomo/sing-box families. |
| `WireGuard` | ✅ | ✅ | ✅ | ⚠️ | ✅ | ◐ | Strong target overall, but export shape differs across families. |
| `Snell` | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ | Only target where it is clearly documented. |
| `AnyTLS` | ⚠️ | ✅ | ✅ | ✅ | ✅ | ◐ | Mihomo supports AnyTLS but explicitly not `AnyTLS + Reality`. |
| `NaiveProxy` | ❌ | ❌ | ❌ | ❌ | ✅ | ◐ | sing-box-first capability; Shadowrocket has official release-note evidence. |
| `SSH` | ❌ | ✅ | ✅ | ❌ | ✅ | ❌ | Not a v1 renderer priority for Mihomo/Xray families. |
| `ShadowTLS` | ⚠️ | ⚠️ | ⚠️ | ❌ | ✅ | ❌ | Usually modeled as an obfuscation layer or plugin, not a pure standalone node type in every family. |
| `Juicity` | ❌ | ✅ | ❌ | ❌ | ❌ | ❌ | Stash-only first-class target in this matrix. |

## Interpretation Rules

### 1. Treat transport features separately

The following must not be modeled as top-level protocols in the same way as `ss` or `trojan`:

- `Reality`
- `XTLS / Vision`
- `WS`
- `gRPC`
- `HTTP/2`
- `HTTP/3`
- `ShadowTLS`
- `ECH`

These should be modeled as:

- protocol
- transport
- tls features
- optional obfuscation or extension layers

### 1.1 VLESS Reality and AnyTLS handling

`VLESS + Reality` is supported as `protocol = vless` with `security = reality`,
plus Reality fields such as `pbk`, `sid`, `sni`, `fp`, and `flow`.

`AnyTLS` is a standalone protocol target in this system. Do not model
`AnyTLS + Reality` as a supported combination for Mihomo; official Mihomo
documentation explicitly says that combination is not supported and will not be
supported. For Reality, prefer VLESS, VMess, or Trojan transport fields.

### 2. Renderer families we should implement first

Based on the matrix, first-class export targets should be:

1. `mihomo`
2. `xray_uri_bundle`
3. `surge`
4. `sing_box_outbound_bundle`

Client aliases should map to these renderer families:

- `clash-verge-rev` -> `mihomo`
- `stash` -> `mihomo` plus `stash-specific transforms` later
- `v2rayn` -> `xray_uri_bundle`
- `v2rayng` -> `xray_uri_bundle`
- `surge` -> `surge`
- `nekobox` -> `sing_box_outbound_bundle`
- `hiddify` -> `sing_box_outbound_bundle`
- `shadowrocket` -> `surge` or `apple_compat` bridge path first, then dedicated tuning later

### 3. Strict vs best-effort export

For every target family, the backend should support two export modes:

- `strict`
- `best_effort`

Strict:

- fail export if any selected node is incompatible with the target family

Best effort:

- drop unsupported nodes
- include warnings in preview and logs

## Recommended V1 Protocol Set

These are the safest first-wave protocols for broad mainstream client coverage:

- `Shadowsocks`
- `VMess`
- `VLESS`
- `Trojan`
- `Hysteria2`
- `TUIC`
- `WireGuard`

Compatibility carry-over only:

- `ShadowsocksR`
- `Hysteria 1`
- `Snell`

Specialized / staged later:

- `AnyTLS`
- `NaiveProxy`
- `SSH`
- `Juicity`
- `ShadowTLS`

## Design Impact On Backend

Because of the matrix, the Rust backend should model:

- protocol identity
- transport identity
- tls capability
- udp capability
- renderer compatibility
- client-family overrides

Each protocol handler should expose something like:

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

## Design Impact On Frontend

The Vue 3 admin should show:

- target client family selector
- node compatibility badges
- unsupported node warnings
- strict vs best-effort switch
- per-subscription default export target

## Source Notes

This matrix is based on official sources checked on 2026-05-29.

Primary sources used:

- [Mihomo docs](https://wiki.metacubex.one/en/config/)
- [Mihomo Shadowsocks](https://wiki.metacubex.one/en/config/proxies/ss/)
- [Mihomo TUIC](https://wiki.metacubex.one/en/config/proxies/tuic/)
- [Mihomo WireGuard](https://wiki.metacubex.one/en/config/proxies/wg/)
- [Mihomo AnyTLS](https://wiki.metacubex.one/en/config/proxies/anytls/)
- [Mihomo Snell](https://wiki.metacubex.one/en/config/proxies/snell/)
- [Stash protocol types](https://stash.wiki/en/proxy-protocols/proxy-types)
- [Stash release notes](https://stash.wiki/en/release-notes/ios)
- [Surge proxy policy](https://manual.nssurge.com/policy/proxy.html)
- [Surge start page](https://manual.nssurge.com/)
- [v2rayN subscription description](https://github.com/2dust/v2rayN/wiki/Description-of-subscription)
- [v2rayN release files introduction](https://github.com/2dust/v2rayN/wiki/Release-files-introduction)
- [v2rayNG repository](https://github.com/2dust/v2rayNG)
- [sing-box outbound types](https://sing-box.sagernet.org/configuration/outbound/)
- [sing-box Shadowsocks outbound](https://sing-box.sagernet.org/configuration/outbound/shadowsocks/)
- [sing-box SOCKS outbound](https://sing-box.sagernet.org/configuration/outbound/socks/)
- [sing-box HTTP outbound](https://sing-box.sagernet.org/configuration/outbound/http/)
- [NekoBox for Android](https://github.com/MatsuriDayo/NekoBoxForAndroid)
- [Hiddify App](https://github.com/hiddify/hiddify-app)
- [Shadowrocket App Store page](https://apps.apple.com/us/app/shadowrocket/id932747118?l=en-us)

Where a cell is marked `◐`, the classification is based on official app-store release notes or broad official product metadata rather than a complete protocol specification page.
