use crate::db::DbPool;

use crate::{
    repository::template_repo::{self, NewTemplateRecord},
    utils::time::now_rfc3339,
};

struct DefaultTemplate {
    name: &'static str,
    kind: &'static str,
    content: &'static str,
}

const CLASH_TEMPLATE: &str = r#"mixed-port: 7890
allow-lan: true
mode: rule
log-level: info
profile:
  store-selected: true
  store-fake-ip: true
dns:
  enable: true
  ipv6: false
proxy-groups:
  - name: 节点选择
    type: select
    include-all-proxies: true
    proxies:
      - 自动选择
      - DIRECT
  - name: 自动选择
    type: url-test
    include-all-proxies: true
    url: https://www.gstatic.com/generate_204
    interval: 300
rules:
  - GEOIP,CN,DIRECT
  - MATCH,节点选择
"#;

const MIHOMO_TEMPLATE: &str = r#"mixed-port: 7890
allow-lan: true
mode: rule
log-level: info
unified-delay: true
tcp-concurrent: true
profile:
  store-selected: true
proxy-groups:
  - name: PROXY
    type: select
    include-all-proxies: true
    proxies:
      - AUTO
      - DIRECT
  - name: AUTO
    type: url-test
    include-all-proxies: true
    url: https://www.gstatic.com/generate_204
    interval: 300
rules:
  - GEOIP,CN,DIRECT
  - MATCH,PROXY
"#;

const SING_BOX_TEMPLATE: &str = r#"{
  "log": {
    "level": "info"
  },
  "dns": {
    "servers": [
      {
        "tag": "remote",
        "address": "https://1.1.1.1/dns-query"
      },
      {
        "tag": "local",
        "address": "223.5.5.5"
      }
    ],
    "final": "remote"
  },
  "inbounds": [
    {
      "type": "mixed",
      "tag": "mixed-in",
      "listen": "127.0.0.1",
      "listen_port": 2080
    }
  ],
  "outbounds": [
    {
      "type": "direct",
      "tag": "direct"
    },
    {
      "type": "block",
      "tag": "block"
    }
  ],
  "route": {
    "auto_detect_interface": true,
    "rules": [
      {
        "geoip": "cn",
        "outbound": "direct"
      }
    ],
    "final": "select"
  }
}"#;

const SURGE_TEMPLATE: &str = r#"#!MANAGED-CONFIG https://example.com/surge.conf interval=86400 strict=false

[General]
loglevel = notify
ipv6 = false
skip-proxy = 192.168.0.0/16, 10.0.0.0/8, 172.16.0.0/12, localhost, *.local
bypass-system = true

[Proxy]

[Proxy Group]

[Rule]
DOMAIN-SUFFIX,local,DIRECT
GEOIP,CN,DIRECT
FINAL,Proxy
"#;

const SURGE3_TEMPLATE: &str = r#"#!MANAGED-CONFIG https://example.com/surge3.conf interval=86400 strict=false

[General]
loglevel = notify
ipv6 = false

[Proxy]

[Proxy Group]

[Rule]
GEOIP,CN,DIRECT
FINAL,Proxy
"#;

const SURGE2_TEMPLATE: &str = r#"#!MANAGED-CONFIG https://example.com/surge2.conf interval=86400 strict=false

[General]
loglevel = notify

[Proxy]

[Proxy Group]

[Rule]
FINAL,Proxy
"#;

const XRAY_TEMPLATE: &str = r#"# Xray / V2Ray URI bundle
# This target exports one URI per line. Template content is kept as operator notes.
"#;

const QUANX_TEMPLATE: &str = r#"[general]
server_check_url = https://www.gstatic.com/generate_204
geo_location_checker = http://ip-api.com/json/?lang=zh-CN, https://raw.githubusercontent.com/KOP-XIAO/QuantumultX/master/Scripts/IP_API.js

[server_remote]

[policy]
static=节点选择, 自动选择, direct, img-url=https://raw.githubusercontent.com/Koolson/Qure/master/IconSet/Color/Proxy.png
url-latency-benchmark=自动选择, server-tag-regex=.*, check-interval=600, tolerance=0, alive-checking=false, img-url=https://raw.githubusercontent.com/Koolson/Qure/master/IconSet/Color/Auto.png

[filter_remote]

[rewrite_remote]

[task_local]
"#;

const QUAN_TEMPLATE: &str = r#"[SERVER]

[POLICY]
static=PROXY, auto, direct
url-latency-benchmark=auto, server-tag-regex=.*, check-interval=600

[FILTER]
geoip, cn, direct
final, PROXY
"#;

const LOON_TEMPLATE: &str = r#"[General]
skip-proxy = 192.168.0.0/16, 10.0.0.0/8, 172.16.0.0/12, localhost, *.local

[Proxy]

[Remote Proxy]

[Proxy Group]
节点选择 = select, 自动选择, DIRECT
自动选择 = url-test, url = https://www.gstatic.com/generate_204, interval = 600

[Rule]
GEOIP,CN,DIRECT
FINAL,节点选择
"#;

const SURFBOARD_TEMPLATE: &str = r#"[General]
loglevel = notify

[Proxy]

[Proxy Group]
Proxy = select, Auto, DIRECT
Auto = url-test, url=http://www.gstatic.com/generate_204, interval=600

[Rule]
GEOIP,CN,DIRECT
FINAL,Proxy
"#;

const MELLOW_TEMPLATE: &str = r#"port: 7890
socks-port: 7891
allow-lan: true
mode: rule
log-level: info
proxy-groups:
  - name: PROXY
    type: select
    include-all-proxies: true
    proxies:
      - DIRECT
rules:
  - GEOIP,CN,DIRECT
  - MATCH,PROXY
"#;

const CLASHR_TEMPLATE: &str = r#"mixed-port: 7890
allow-lan: true
mode: rule
log-level: info
proxy-groups:
  - name: PROXY
    type: select
    include-all-proxies: true
    proxies:
      - DIRECT
rules:
  - GEOIP,CN,DIRECT
  - MATCH,PROXY
"#;

const URI_NOTE_TEMPLATE: &str = r#"# URI bundle target
# This target exports compatible raw node links, one per line.
"#;

const SSSUB_TEMPLATE: &str = r#"{
  "version": 1,
  "remarks": "sublinkx-rs SIP008 template",
  "servers": []
}"#;

const SSD_TEMPLATE: &str = r#"{
  "airport": "sublinkx-rs",
  "port": 0,
  "encryption": "",
  "password": "",
  "servers": []
}"#;

const DEFAULT_TEMPLATES: &[DefaultTemplate] = &[
    DefaultTemplate {
        name: "Built-in Common Notes",
        kind: "common",
        content: "# Common template notes\n",
    },
    DefaultTemplate {
        name: "Built-in Clash ACL4SSR Style",
        kind: "clash",
        content: CLASH_TEMPLATE,
    },
    DefaultTemplate {
        name: "Built-in Mihomo Rule Base",
        kind: "mihomo",
        content: MIHOMO_TEMPLATE,
    },
    DefaultTemplate {
        name: "Built-in Xray URI Bundle",
        kind: "xray",
        content: XRAY_TEMPLATE,
    },
    DefaultTemplate {
        name: "Built-in Surge 4/5 Managed",
        kind: "surge",
        content: SURGE_TEMPLATE,
    },
    DefaultTemplate {
        name: "Built-in sing-box Route Base",
        kind: "sing-box",
        content: SING_BOX_TEMPLATE,
    },
    DefaultTemplate {
        name: "Built-in Surge 3 Managed",
        kind: "surge3",
        content: SURGE3_TEMPLATE,
    },
    DefaultTemplate {
        name: "Built-in Surge 2 Managed",
        kind: "surge2",
        content: SURGE2_TEMPLATE,
    },
    DefaultTemplate {
        name: "Built-in Quantumult X Base",
        kind: "quanx",
        content: QUANX_TEMPLATE,
    },
    DefaultTemplate {
        name: "Built-in Quantumult Base",
        kind: "quan",
        content: QUAN_TEMPLATE,
    },
    DefaultTemplate {
        name: "Built-in Loon Base",
        kind: "loon",
        content: LOON_TEMPLATE,
    },
    DefaultTemplate {
        name: "Built-in Surfboard Base",
        kind: "surfboard",
        content: SURFBOARD_TEMPLATE,
    },
    DefaultTemplate {
        name: "Built-in Mellow Base",
        kind: "mellow",
        content: MELLOW_TEMPLATE,
    },
    DefaultTemplate {
        name: "Built-in ClashR Base",
        kind: "clashr",
        content: CLASHR_TEMPLATE,
    },
    DefaultTemplate {
        name: "Built-in Shadowsocks SIP002 Notes",
        kind: "ss",
        content: URI_NOTE_TEMPLATE,
    },
    DefaultTemplate {
        name: "Built-in Shadowsocks SIP008 Base",
        kind: "sssub",
        content: SSSUB_TEMPLATE,
    },
    DefaultTemplate {
        name: "Built-in ShadowsocksR Notes",
        kind: "ssr",
        content: URI_NOTE_TEMPLATE,
    },
    DefaultTemplate {
        name: "Built-in ShadowsocksD Base",
        kind: "ssd",
        content: SSD_TEMPLATE,
    },
    DefaultTemplate {
        name: "Built-in Trojan URI Notes",
        kind: "trojan",
        content: URI_NOTE_TEMPLATE,
    },
    DefaultTemplate {
        name: "Built-in Mixed URI Notes",
        kind: "mixed",
        content: URI_NOTE_TEMPLATE,
    },
];

pub async fn seed_default_templates(pool: &DbPool) -> Result<(), sqlx::Error> {
    let now = now_rfc3339();

    for template in DEFAULT_TEMPLATES {
        if template_repo::find_by_name(pool, template.name)
            .await?
            .is_some()
        {
            continue;
        }

        template_repo::insert(
            pool,
            &NewTemplateRecord {
                name: template.name,
                kind: template.kind,
                content: template.content,
                created_at: &now,
                updated_at: &now,
            },
        )
        .await?;
    }

    Ok(())
}
