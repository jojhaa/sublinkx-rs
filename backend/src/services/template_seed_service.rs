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

pub(crate) const CLASH_TEMPLATE: &str = r#"mixed-port: 7890
allow-lan: true
mode: rule
log-level: info
ipv6: false
profile:
  store-selected: true
  store-fake-ip: true
dns:
  enable: true
  ipv6: false
  listen: 0.0.0.0:1053
  enhanced-mode: fake-ip
  nameserver:
    - https://223.5.5.5/dns-query
    - https://doh.pub/dns-query
proxy-groups:
  - name: 节点选择
    type: select
    proxies:
      - 手动切换
      - 自动选择
      - DIRECT
  - name: 手动切换
    type: select
    include-all-proxies: true
  - name: 自动选择
    type: url-test
    include-all-proxies: true
    url: https://cp.cloudflare.com/generate_204
    interval: 300
    tolerance: 50
  - name: Ai平台
    type: select
    proxies:
      - 节点选择
      - 自动选择
      - 手动切换
      - DIRECT
  - name: 油管视频
    type: select
    proxies:
      - 节点选择
      - 自动选择
      - 手动切换
  - name: 奈飞视频
    type: select
    proxies:
      - 节点选择
      - 自动选择
      - 手动切换
  - name: 电报消息
    type: select
    proxies:
      - 节点选择
      - 自动选择
      - 手动切换
      - DIRECT
  - name: 微软服务
    type: select
    proxies:
      - DIRECT
      - 节点选择
      - 手动切换
  - name: 苹果服务
    type: select
    proxies:
      - DIRECT
      - 节点选择
      - 手动切换
  - name: 全球直连
    type: select
    proxies:
      - DIRECT
      - 节点选择
  - name: 广告拦截
    type: select
    proxies:
      - REJECT
      - DIRECT
  - name: 漏网之鱼
    type: select
    proxies:
      - 节点选择
      - 自动选择
      - DIRECT
rule-providers:
  localarea:
    type: http
    behavior: classical
    url: https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/LocalAreaNetwork.list
    path: ./ruleset/localarea.yaml
    interval: 86400
  unban:
    type: http
    behavior: classical
    url: https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/UnBan.list
    path: ./ruleset/unban.yaml
    interval: 86400
  banad:
    type: http
    behavior: classical
    url: https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/BanAD.list
    path: ./ruleset/banad.yaml
    interval: 86400
  googlecn:
    type: http
    behavior: classical
    url: https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/GoogleCN.list
    path: ./ruleset/googlecn.yaml
    interval: 86400
  apple:
    type: http
    behavior: classical
    url: https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/Apple.list
    path: ./ruleset/apple.yaml
    interval: 86400
  microsoft:
    type: http
    behavior: classical
    url: https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/Microsoft.list
    path: ./ruleset/microsoft.yaml
    interval: 86400
  telegram:
    type: http
    behavior: classical
    url: https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/Telegram.list
    path: ./ruleset/telegram.yaml
    interval: 86400
  ai:
    type: http
    behavior: classical
    url: https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/Ruleset/AI.list
    path: ./ruleset/ai.yaml
    interval: 86400
  openai:
    type: http
    behavior: classical
    url: https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/Ruleset/OpenAi.list
    path: ./ruleset/openai.yaml
    interval: 86400
  youtube:
    type: http
    behavior: classical
    url: https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/Ruleset/YouTube.list
    path: ./ruleset/youtube.yaml
    interval: 86400
  netflix:
    type: http
    behavior: classical
    url: https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/Ruleset/Netflix.list
    path: ./ruleset/netflix.yaml
    interval: 86400
  proxygfw:
    type: http
    behavior: classical
    url: https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/ProxyGFWlist.list
    path: ./ruleset/proxygfw.yaml
    interval: 86400
  chinadomain:
    type: http
    behavior: classical
    url: https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/ChinaDomain.list
    path: ./ruleset/chinadomain.yaml
    interval: 86400
  download:
    type: http
    behavior: classical
    url: https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/Download.list
    path: ./ruleset/download.yaml
    interval: 86400
rules:
  - RULE-SET,localarea,全球直连
  - RULE-SET,unban,全球直连
  - RULE-SET,banad,广告拦截
  - DOMAIN-SUFFIX,chatgpt.com,Ai平台
  - DOMAIN-SUFFIX,openai.com,Ai平台
  - DOMAIN-SUFFIX,anthropic.com,Ai平台
  - DOMAIN-SUFFIX,claude.ai,Ai平台
  - DOMAIN-SUFFIX,github.com,节点选择
  - DOMAIN-SUFFIX,githubusercontent.com,节点选择
  - DOMAIN-SUFFIX,githubassets.com,节点选择
  - DOMAIN-SUFFIX,youtube.com,油管视频
  - DOMAIN-SUFFIX,googlevideo.com,油管视频
  - DOMAIN-SUFFIX,ytimg.com,油管视频
  - DOMAIN-SUFFIX,netflix.com,奈飞视频
  - DOMAIN-SUFFIX,nflxvideo.net,奈飞视频
  - DOMAIN-SUFFIX,t.me,电报消息
  - DOMAIN-SUFFIX,telegram.org,电报消息
  - DOMAIN-SUFFIX,x.com,节点选择
  - DOMAIN-SUFFIX,twitter.com,节点选择
  - DOMAIN-SUFFIX,instagram.com,节点选择
  - DOMAIN-SUFFIX,tiktok.com,节点选择
  - DOMAIN-SUFFIX,spotify.com,节点选择
  - DOMAIN-SUFFIX,disneyplus.com,节点选择
  - DOMAIN-SUFFIX,primevideo.com,节点选择
  - DOMAIN-SUFFIX,max.com,节点选择
  - DOMAIN-SUFFIX,hbomax.com,节点选择
  - DOMAIN-SUFFIX,steamcommunity.com,节点选择
  - DOMAIN-SUFFIX,steampowered.com,节点选择
  - DOMAIN-SUFFIX,epicgames.com,节点选择
  - RULE-SET,googlecn,全球直连
  - RULE-SET,apple,苹果服务
  - RULE-SET,microsoft,微软服务
  - RULE-SET,telegram,电报消息
  - RULE-SET,ai,Ai平台
  - RULE-SET,openai,Ai平台
  - RULE-SET,youtube,油管视频
  - RULE-SET,netflix,奈飞视频
  - RULE-SET,proxygfw,节点选择
  - RULE-SET,chinadomain,全球直连
  - RULE-SET,download,全球直连
  - MATCH,漏网之鱼
"#;

pub(crate) const MIHOMO_TEMPLATE: &str = r#"mixed-port: 7890
allow-lan: true
mode: rule
log-level: info
unified-delay: true
tcp-concurrent: true
profile:
  store-selected: true
# Add fixed domain records here only when the address is controlled and stable.
hosts: {}
dns:
  enable: true
  cache-algorithm: arc
  use-hosts: true
  use-system-hosts: true
  listen: 0.0.0.0:1053
  ipv6: false
  enhanced-mode: fake-ip
  fake-ip-filter:
    - rule-set:private_domain
    - "+.lan"
    - "+.local"
    - localhost
    - time.windows.com
    - time.apple.com
    - time.google.com
    - localhost.ptlogin2.qq.com
    - dns.msftncsi.com
    - www.msftconnecttest.com
    - captive.apple.com
    - connectivitycheck.gstatic.com
  fake-ip-filter-mode: blacklist
  default-nameserver:
    - 223.5.5.5
    - 119.29.29.29
  nameserver:
    - https://1.1.1.1/dns-query#代理选择
    - https://8.8.8.8/dns-query#代理选择
  nameserver-policy:
    "rule-set:private_domain":
      - https://dns.alidns.com/dns-query
      - https://doh.pub/dns-query
    "rule-set:cn_domain":
      - https://dns.alidns.com/dns-query
      - https://doh.pub/dns-query
    "rule-set:geolocation-not-cn":
      - https://1.1.1.1/dns-query#代理选择
      - https://8.8.8.8/dns-query#代理选择
  proxy-server-nameserver:
    - https://dns.alidns.com/dns-query
    - https://doh.pub/dns-query
  respect-rules: false
  direct-nameserver:
    - https://dns.alidns.com/dns-query
    - https://doh.pub/dns-query
  direct-nameserver-follow-policy: true
proxy-groups:
  - name: 代理选择
    type: select
    proxies:
      - 手动选择
      - 自动选择
      - 故障转移
      - 负载均衡
      - DIRECT
  - name: 手动选择
    type: select
    include-all-proxies: true
  - name: 自动选择
    type: url-test
    include-all-proxies: true
    url: https://cp.cloudflare.com/generate_204
    interval: 300
    tolerance: 50
  - name: 故障转移
    type: fallback
    include-all-proxies: true
    url: https://cp.cloudflare.com/generate_204
    interval: 300
    lazy: true
  - name: 负载均衡
    type: load-balance
    strategy: consistent-hashing
    include-all-proxies: true
    url: https://cp.cloudflare.com/generate_204
    interval: 300
    lazy: true
  - name: AI 服务
    type: select
    proxies:
      - 代理选择
      - 自动选择
      - 手动选择
      - DIRECT
  - name: 流媒体
    type: select
    proxies:
      - 代理选择
      - 自动选择
      - 手动选择
      - DIRECT
  - name: 谷歌服务
    type: select
    proxies:
      - 代理选择
      - 自动选择
      - 手动选择
      - DIRECT
  - name: 电报消息
    type: select
    proxies:
      - 代理选择
      - 自动选择
      - 手动选择
      - DIRECT
  - name: 微软服务
    type: select
    proxies:
      - DIRECT
      - 代理选择
      - 手动选择
  - name: 苹果服务
    type: select
    proxies:
      - DIRECT
      - 代理选择
      - 手动选择
  - name: 国内网站
    type: select
    proxies:
      - DIRECT
      - 代理选择
  - name: 游戏平台
    type: select
    proxies:
      - 代理选择
      - 手动选择
      - 自动选择
      - DIRECT
  - name: 下载拦截
    type: select
    proxies:
      - REJECT
      - DIRECT
  - name: 漏网之鱼
    type: select
    proxies:
      - 代理选择
      - 自动选择
      - DIRECT
rule-providers:
  private_domain:
    type: http
    interval: 86400
    behavior: domain
    format: mrs
    url: https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geosite/private.mrs
  private_ip:
    type: http
    interval: 86400
    behavior: ipcidr
    format: mrs
    url: https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geoip/private.mrs
  ai:
    type: http
    interval: 86400
    behavior: domain
    format: mrs
    url: https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geosite/category-ai-!cn.mrs
  github_domain:
    type: http
    interval: 86400
    behavior: domain
    format: mrs
    url: https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geosite/github.mrs
  youtube_domain:
    type: http
    interval: 86400
    behavior: domain
    format: mrs
    url: https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geosite/youtube.mrs
  google_domain:
    type: http
    interval: 86400
    behavior: domain
    format: mrs
    url: https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geosite/google.mrs
  telegram_domain:
    type: http
    interval: 86400
    behavior: domain
    format: mrs
    url: https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geosite/telegram.mrs
  telegram_ip:
    type: http
    interval: 86400
    behavior: ipcidr
    format: mrs
    url: https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geoip/telegram.mrs
  netflix_domain:
    type: http
    interval: 86400
    behavior: domain
    format: mrs
    url: https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geosite/netflix.mrs
  netflix_ip:
    type: http
    interval: 86400
    behavior: ipcidr
    format: mrs
    url: https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geoip/netflix.mrs
  bilibili_domain:
    type: http
    interval: 86400
    behavior: domain
    format: mrs
    url: https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geosite/bilibili.mrs
  spotify_domain:
    type: http
    interval: 86400
    behavior: domain
    format: mrs
    url: https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geosite/spotify.mrs
  steam_domain:
    type: http
    interval: 86400
    behavior: domain
    format: mrs
    url: https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geosite/steam.mrs
  paypal_domain:
    type: http
    interval: 86400
    behavior: domain
    format: mrs
    url: https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geosite/paypal.mrs
  onedrive_domain:
    type: http
    interval: 86400
    behavior: domain
    format: mrs
    url: https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geosite/onedrive.mrs
  microsoft_domain:
    type: http
    interval: 86400
    behavior: domain
    format: mrs
    url: https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geosite/microsoft.mrs
  apple_domain:
    type: http
    interval: 86400
    behavior: domain
    format: mrs
    url: https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geosite/apple.mrs
  apple_ip:
    type: http
    interval: 86400
    behavior: ipcidr
    format: mrs
    url: https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo-lite/geoip/apple.mrs
  geolocation-not-cn:
    type: http
    interval: 86400
    behavior: domain
    format: mrs
    url: https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geosite/geolocation-!cn.mrs
  cn_domain:
    type: http
    interval: 86400
    behavior: domain
    format: mrs
    url: https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geosite/cn.mrs
  cn_ip:
    type: http
    interval: 86400
    behavior: ipcidr
    format: mrs
    url: https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geoip/cn.mrs
  tracker_domain:
    type: http
    interval: 86400
    behavior: domain
    format: mrs
    url: https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geosite/tracker.mrs
  private_tracker_domain:
    type: http
    interval: 86400
    behavior: domain
    format: mrs
    url: https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geosite/category-pt.mrs
rules:
  - RULE-SET,private_ip,DIRECT,no-resolve
  - RULE-SET,private_domain,DIRECT
  - PROCESS-NAME-WILDCARD,*torrent*,下载拦截
  - PROCESS-NAME,qbittorrent,下载拦截
  - PROCESS-NAME,qbittorrent.exe,下载拦截
  - PROCESS-NAME,transmission-daemon,下载拦截
  - PROCESS-NAME,transmission-qt,下载拦截
  - PROCESS-NAME,deluge,下载拦截
  - PROCESS-NAME,deluged,下载拦截
  - PROCESS-NAME,aria2c,下载拦截
  - PROCESS-NAME,motrix,下载拦截
  - PROCESS-NAME,Thunder.exe,下载拦截
  - PROCESS-NAME,DownloadSDKServer.exe,下载拦截
  - RULE-SET,tracker_domain,下载拦截
  - RULE-SET,private_tracker_domain,下载拦截
  - DST-PORT,6881-6999,下载拦截
  - DST-PORT,51413,下载拦截
  - DOMAIN-SUFFIX,chatgpt.com,AI 服务
  - DOMAIN-SUFFIX,openai.com,AI 服务
  - DOMAIN-SUFFIX,anthropic.com,AI 服务
  - DOMAIN-SUFFIX,claude.ai,AI 服务
  - DOMAIN-SUFFIX,github.com,代理选择
  - DOMAIN-SUFFIX,githubusercontent.com,代理选择
  - DOMAIN-SUFFIX,githubassets.com,代理选择
  - DOMAIN-SUFFIX,youtube.com,流媒体
  - DOMAIN-SUFFIX,googlevideo.com,流媒体
  - DOMAIN-SUFFIX,ytimg.com,流媒体
  - DOMAIN-SUFFIX,netflix.com,流媒体
  - DOMAIN-SUFFIX,nflxvideo.net,流媒体
  - DOMAIN-SUFFIX,t.me,电报消息
  - DOMAIN-SUFFIX,telegram.org,电报消息
  - DOMAIN-SUFFIX,x.com,代理选择
  - DOMAIN-SUFFIX,twitter.com,代理选择
  - DOMAIN-SUFFIX,instagram.com,代理选择
  - DOMAIN-SUFFIX,tiktok.com,流媒体
  - DOMAIN-SUFFIX,spotify.com,流媒体
  - DOMAIN-SUFFIX,disneyplus.com,流媒体
  - DOMAIN-SUFFIX,primevideo.com,流媒体
  - DOMAIN-SUFFIX,max.com,流媒体
  - DOMAIN-SUFFIX,hbomax.com,流媒体
  - DOMAIN-SUFFIX,steamcommunity.com,游戏平台
  - DOMAIN-SUFFIX,steampowered.com,游戏平台
  - DOMAIN-SUFFIX,epicgames.com,游戏平台
  - RULE-SET,ai,AI 服务
  - RULE-SET,github_domain,代理选择
  - RULE-SET,youtube_domain,流媒体
  - RULE-SET,google_domain,谷歌服务
  - RULE-SET,telegram_domain,电报消息
  - RULE-SET,telegram_ip,电报消息,no-resolve
  - RULE-SET,netflix_domain,流媒体
  - RULE-SET,netflix_ip,流媒体,no-resolve
  - RULE-SET,bilibili_domain,国内网站
  - RULE-SET,spotify_domain,流媒体
  - RULE-SET,steam_domain,游戏平台
  - RULE-SET,paypal_domain,代理选择
  - RULE-SET,onedrive_domain,微软服务
  - RULE-SET,microsoft_domain,微软服务
  - RULE-SET,apple_domain,苹果服务
  - RULE-SET,apple_ip,苹果服务,no-resolve
  - RULE-SET,geolocation-not-cn,代理选择
  - RULE-SET,cn_domain,国内网站
  - RULE-SET,cn_ip,国内网站,no-resolve
  - MATCH,漏网之鱼
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
        name: "Built-in Mihomo Policy Orchestrator",
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
                is_builtin: 1,
                created_at: &now,
                updated_at: &now,
            },
        )
        .await?;
    }

    Ok(())
}
