# Third-Party Notices

This file records third-party code and data distributed with SublinkX-RS. It does not imply endorsement by the listed projects or authors.

## MetaCubeX/meta-rules-dat

- Component: selected Mihomo domain and IP rule payloads
- Upstream project: https://github.com/MetaCubeX/meta-rules-dat
- Snapshot commit: `bfcdd67a88b5c3412879d8680e7b101e59b352f5`
- License: GNU General Public License v3.0 only (`GPL-3.0-only`)
- Local license copy: `licenses/rules/MetaCubeX-meta-rules-dat-GPL-3.0.txt`
- Local source manifest: `backend/assets/mihomo-rules-manifest.json`
- Modification: 23 upstream YAML `payload` lists are selected and merged into one generated Mihomo `inline` rule-provider snapshot. Large CN and proxy categories use the upstream `geo-lite` common collections. Rule entries are not rewritten.
- Usage: the generated snapshot is embedded in the backend and copied into exports made from the built-in Mihomo policy template. Custom templates and upstream passthrough templates are not replaced.
- Attribution or NOTICE: retain this notice, the manifest, the update script, and the GPL-3.0 license copy when redistributing the bundled snapshot.
- Source availability: the exact upstream source URLs and SHA-256 values are recorded in the manifest; `scripts/update-mihomo-rules.ps1` is the corresponding reproducible transformation script.

The upstream dataset aggregates material from multiple sources. Review the upstream project documentation and source notices before changing the selected datasets or redistribution model.
