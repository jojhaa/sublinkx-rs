# Client Compatibility Targets

As of 2026-05-29, the rewrite should target current mainstream proxy clients instead of only preserving legacy `clash / surge / v2ray` outputs.

This document defines compatibility priorities for `sublinkx-rs`.

## 1. Compatibility Strategy

The new system should not treat "subscription export" as a single feature.

It should support multiple output families:

- Mihomo / Clash-family configuration
- Xray / v2rayN-family node subscription
- sing-box outbound subscription
- Surge-family configuration

The backend architecture must support:

- Multiple renderers
- Capability-aware protocol filtering
- Client-specific profile generation
- Format versioning per client family

## 2. Priority Tiers

### Tier 1: Must Support In MVP Or Early Milestones

These are the first-class compatibility targets.

1. Mihomo / Clash-family
   - Representative clients:
   - Clash Verge Rev
   - Stash
   - Other Mihomo-compatible GUIs

2. v2rayN / Xray-family
   - Representative clients:
   - v2rayN
   - v2rayNG

3. Surge-family
   - Representative clients:
   - Surge
   - Shadowrocket partial compatibility path via Surge-style output when applicable

4. sing-box-family
   - Representative clients:
   - NekoBox for Android
   - Hiddify App
   - Other sing-box GUI clients

### Tier 2: Planned After Core Stability

- Direct Shadowrocket-optimized output
- sing-box rule-set aware profile output
- GUI.for.SingBox-oriented compatibility verification
- Hiddify-specific profile tuning

## 3. Current Mainstream Client Signals

The following projects/pages were checked on 2026-05-29:

- Clash Verge Rev is actively maintained and positions itself as a modern GUI built around Mihomo for Windows, macOS, and Linux.
- v2rayN is actively maintained and describes itself as a GUI client for Windows, Linux, and macOS supporting Xray, sing-box, and others.
- v2rayNG is actively maintained and describes itself as an Android client supporting Xray core and v2fly core.
- NekoBox for Android is actively maintained and documents support for widely used subscription formats including ClashMeta, v2rayN, and sing-box outbound.
- Hiddify App positions itself as a multi-platform client based on sing-box.
- Stash documents itself as a Clash-compatible client on Apple platforms.
- Shadowrocket remains a widely used Apple-platform rule-based proxy client.

## 4. Renderer Targets

The backend should provide separate renderers instead of one mixed export path:

- `mihomo`
- `surge`
- `xray_uri_bundle`
- `sing_box_outbound_bundle`

Recommended public access form:

- `/s/{token}?target=mihomo`
- `/s/{token}?target=surge`
- `/s/{token}?target=xray`
- `/s/{token}?target=sing-box`

Optional alias mapping may be added later:

- `client=clash-verge-rev`
- `client=v2rayn`
- `client=v2rayng`
- `client=nekobox`
- `client=hiddify`
- `client=stash`
- `client=shadowrocket`

The backend should internally map client aliases to renderer families plus profile transforms.

## 5. Compatibility Matrix Design

Each protocol handler should declare render support:

- Can render to Mihomo
- Can render to Surge
- Can render to Xray URI bundle
- Can render to sing-box outbound

This avoids invalid exports for unsupported protocol and client combinations.

Example:

- `wireguard` may support sing-box but not all Surge variants
- `reality` transport may work for sing-box and Xray-family clients but need client-specific flags
- Some advanced plugins may be acceptable only in Mihomo-compatible outputs

## 6. Product Rules

Rules for export behavior:

1. Never silently emit unsupported nodes into a client format.
2. Return explicit compatibility warnings in admin preview.
3. Allow per-subscription client presets.
4. Allow per-client filtering rules.
5. Support "strict mode" and "best effort mode".

Strict mode:

- Fails when any selected node is not compatible with the target client family.

Best effort mode:

- Drops unsupported nodes and records warnings.

## 7. Frontend Requirements

The admin frontend should expose:

- Target client family selector
- Compatibility preview per node
- Unsupported-node warning list
- Export sample preview
- Default client target per subscription

## 8. Delivery Order

Recommended implementation order:

1. Mihomo renderer
2. Xray URI bundle renderer for v2rayN / v2rayNG class clients
3. Surge renderer
4. sing-box outbound renderer
5. Client-specific compatibility tuning

## 9. Sources

Sources checked on 2026-05-29:

- [Clash Verge Rev](https://github.com/clash-verge-rev/clash-verge-rev)
- [v2rayN](https://github.com/2dust/v2rayN)
- [v2rayNG](https://github.com/2dust/v2rayNG)
- [NekoBox for Android](https://github.com/MatsuriDayo/NekoBoxForAndroid)
- [Hiddify App](https://github.com/hiddify/hiddify-app)
- [Stash Docs](https://stash.wiki/)
- [Stash App Store page](https://apps.apple.com/us/app/stash/id1596063349?l=zh-CN&l=zh-Hans-CN%3Fplatform%3Diphone)
- [Shadowrocket App Store page](https://apps.apple.com/us/app/shadowrocket/id932747118?l=en-us)

See also:

- [protocol-client-matrix.md](D:\Desktop\tool\sublinkx-rs\docs\protocol-client-matrix.md)
