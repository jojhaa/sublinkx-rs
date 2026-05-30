# Clash Routing Template

Clash/Mihomo routing is built from two parts:

- `proxy-groups`: named policies that choose the final outbound node.
- `rules`: ordered match rules that send traffic to a policy group, `DIRECT`, or `REJECT`.

In sublinkx-rs, the exporter injects real nodes into `proxies`, then appends an `AUTO` select group and a final `MATCH,AUTO` fallback. A Clash template can define additional groups, rule providers, DNS, and routing rules before that fallback.

## Minimal Split Routing Template

Create a template with `kind = clash` and paste this YAML:

```yaml
mixed-port: 7890
allow-lan: true
mode: rule
log-level: info
ipv6: false

dns:
  enable: true
  listen: 0.0.0.0:1053
  enhanced-mode: fake-ip
  nameserver:
    - https://223.5.5.5/dns-query
    - https://doh.pub/dns-query
  fallback:
    - https://1.1.1.1/dns-query
    - https://8.8.8.8/dns-query
  fallback-filter:
    geoip: true
    geoip-code: CN

proxy-groups:
  - name: PROXY
    type: select
    proxies:
      - AUTO
      - DIRECT

  - name: AI
    type: select
    proxies:
      - PROXY
      - AUTO
      - DIRECT

  - name: STREAMING
    type: select
    proxies:
      - PROXY
      - AUTO

  - name: GLOBAL
    type: select
    proxies:
      - PROXY
      - DIRECT

rule-providers:
  reject:
    type: http
    behavior: domain
    url: https://raw.githubusercontent.com/Loyalsoldier/clash-rules/release/reject.txt
    path: ./ruleset/reject.yaml
    interval: 86400
  icloud:
    type: http
    behavior: domain
    url: https://raw.githubusercontent.com/Loyalsoldier/clash-rules/release/icloud.txt
    path: ./ruleset/icloud.yaml
    interval: 86400
  apple:
    type: http
    behavior: domain
    url: https://raw.githubusercontent.com/Loyalsoldier/clash-rules/release/apple.txt
    path: ./ruleset/apple.yaml
    interval: 86400
  google:
    type: http
    behavior: domain
    url: https://raw.githubusercontent.com/Loyalsoldier/clash-rules/release/google.txt
    path: ./ruleset/google.yaml
    interval: 86400
  proxy:
    type: http
    behavior: domain
    url: https://raw.githubusercontent.com/Loyalsoldier/clash-rules/release/proxy.txt
    path: ./ruleset/proxy.yaml
    interval: 86400
  direct:
    type: http
    behavior: domain
    url: https://raw.githubusercontent.com/Loyalsoldier/clash-rules/release/direct.txt
    path: ./ruleset/direct.yaml
    interval: 86400
  private:
    type: http
    behavior: domain
    url: https://raw.githubusercontent.com/Loyalsoldier/clash-rules/release/private.txt
    path: ./ruleset/private.yaml
    interval: 86400

rules:
  - RULE-SET,reject,REJECT
  - RULE-SET,private,DIRECT
  - RULE-SET,icloud,DIRECT
  - RULE-SET,apple,DIRECT
  - DOMAIN-SUFFIX,openai.com,AI
  - DOMAIN-SUFFIX,chatgpt.com,AI
  - DOMAIN-SUFFIX,netflix.com,STREAMING
  - DOMAIN-SUFFIX,youtube.com,STREAMING
  - RULE-SET,google,PROXY
  - RULE-SET,proxy,PROXY
  - RULE-SET,direct,DIRECT
  - GEOIP,CN,DIRECT
```

## How Matching Works

Rules are evaluated from top to bottom. The first match wins. That means block rules and private/direct rules should appear before broad proxy rules. The exporter appends `MATCH,AUTO` after template rules, so unmatched traffic still has a fallback.

`rules` send traffic to a group name such as `PROXY`, `AI`, or `STREAMING`. `proxy-groups` define how that group chooses nodes. The generated `AUTO` group includes exported nodes and is safe to reference from your template.

## Practical Group Pattern

Use `PROXY` as the general-purpose group, then create business groups such as `AI`, `STREAMING`, `GAME`, or `DOWNLOAD`. Rules can route special domains to those groups while everything else falls back to `AUTO`.

If a Clash client does not support remote `rule-providers`, replace `RULE-SET` lines with direct rules such as:

```yaml
rules:
  - DOMAIN-SUFFIX,openai.com,AI
  - DOMAIN-SUFFIX,google.com,PROXY
  - GEOIP,CN,DIRECT
```

## ACL4SSR-Style Full Routing Template

`youshandefeiyang/sub-web-modify` does not hardcode Clash YAML directly. It passes remote subconverter `.ini` files to the backend. Its default Clash routing is based on ACL4SSR remote configs such as:

- `ACL4SSR_Online_Full_NoAuto.ini`
- `ACL4SSR_Online_Full.ini`
- `GeneralClashRule.ini`

That style has two key ideas:

- Many `ruleset=PolicyGroup,RuleListUrl` entries route traffic into named policy groups.
- Many `custom_proxy_group=Name\`type\`filter` entries create manual, auto-test, fallback, region, media, AI, and catch-all groups.

For this project we use native Clash/Mihomo YAML templates instead of subconverter ini. The closest native version is:

```yaml
mixed-port: 7890
allow-lan: true
mode: rule
log-level: info
ipv6: false

dns:
  enable: true
  listen: 0.0.0.0:1053
  enhanced-mode: fake-ip
  nameserver:
    - https://223.5.5.5/dns-query
    - https://doh.pub/dns-query
  fallback:
    - https://1.1.1.1/dns-query
    - https://8.8.8.8/dns-query
  fallback-filter:
    geoip: true
    geoip-code: CN

proxy-groups:
  - name: 节点选择
    type: select
    proxies:
      - 自动选择
      - 手动切换
      - 香港节点
      - 台湾节点
      - 狮城节点
      - 日本节点
      - 美国节点
      - 韩国节点
      - DIRECT

  - name: 手动切换
    type: select
    include-all-proxies: true

  - name: 自动选择
    type: url-test
    include-all-proxies: true
    url: https://www.gstatic.com/generate_204
    interval: 300
    tolerance: 50

  - name: 电报消息
    type: select
    proxies:
      - 节点选择
      - 狮城节点
      - 香港节点
      - 台湾节点
      - 日本节点
      - 美国节点
      - 韩国节点
      - 手动切换
      - DIRECT

  - name: Ai平台
    type: select
    proxies:
      - 节点选择
      - 狮城节点
      - 香港节点
      - 台湾节点
      - 日本节点
      - 美国节点
      - 韩国节点
      - 手动切换
      - DIRECT

  - name: 油管视频
    type: select
    proxies:
      - 节点选择
      - 自动选择
      - 狮城节点
      - 香港节点
      - 台湾节点
      - 日本节点
      - 美国节点
      - 韩国节点
      - 手动切换

  - name: 奈飞视频
    type: select
    proxies:
      - 奈飞节点
      - 节点选择
      - 自动选择
      - 狮城节点
      - 香港节点
      - 台湾节点
      - 日本节点
      - 美国节点
      - 韩国节点
      - 手动切换

  - name: 国外媒体
    type: select
    proxies:
      - 节点选择
      - 自动选择
      - 香港节点
      - 台湾节点
      - 狮城节点
      - 日本节点
      - 美国节点
      - 韩国节点
      - 手动切换
      - DIRECT

  - name: 国内媒体
    type: select
    proxies:
      - DIRECT
      - 香港节点
      - 台湾节点
      - 狮城节点
      - 日本节点
      - 手动切换

  - name: 微软服务
    type: select
    proxies:
      - DIRECT
      - 节点选择
      - 美国节点
      - 香港节点
      - 台湾节点
      - 狮城节点
      - 日本节点
      - 韩国节点
      - 手动切换

  - name: 苹果服务
    type: select
    proxies:
      - DIRECT
      - 节点选择
      - 美国节点
      - 香港节点
      - 台湾节点
      - 狮城节点
      - 日本节点
      - 韩国节点
      - 手动切换

  - name: 游戏平台
    type: select
    proxies:
      - DIRECT
      - 节点选择
      - 美国节点
      - 香港节点
      - 台湾节点
      - 狮城节点
      - 日本节点
      - 韩国节点
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

  - name: 应用净化
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
      - 香港节点
      - 台湾节点
      - 狮城节点
      - 日本节点
      - 美国节点
      - 韩国节点
      - 手动切换

  - name: 香港节点
    type: url-test
    include-all-proxies: true
    filter: "(?i)港|hk|hong kong|hongkong"
    url: https://www.gstatic.com/generate_204
    interval: 300
    tolerance: 50

  - name: 日本节点
    type: url-test
    include-all-proxies: true
    filter: "(?i)日本|东京|大阪|jp|japan"
    url: https://www.gstatic.com/generate_204
    interval: 300
    tolerance: 50

  - name: 美国节点
    type: url-test
    include-all-proxies: true
    filter: "(?i)美|us|united states|america|los angeles|san jose|seattle"
    url: https://www.gstatic.com/generate_204
    interval: 300
    tolerance: 150

  - name: 狮城节点
    type: url-test
    include-all-proxies: true
    filter: "(?i)新加坡|狮城|sg|singapore"
    url: https://www.gstatic.com/generate_204
    interval: 300
    tolerance: 50

  - name: 台湾节点
    type: url-test
    include-all-proxies: true
    filter: "(?i)台|tw|taiwan"
    url: https://www.gstatic.com/generate_204
    interval: 300
    tolerance: 50

  - name: 韩国节点
    type: url-test
    include-all-proxies: true
    filter: "(?i)韩|kr|korea|seoul"
    url: https://www.gstatic.com/generate_204
    interval: 300
    tolerance: 50

  - name: 奈飞节点
    type: select
    include-all-proxies: true
    filter: "(?i)nf|netflix|奈飞|解锁|media"

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
  banprogramad:
    type: http
    behavior: classical
    url: https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/BanProgramAD.list
    path: ./ruleset/banprogramad.yaml
    interval: 86400
  googlefcm:
    type: http
    behavior: classical
    url: https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/Ruleset/GoogleFCM.list
    path: ./ruleset/googlefcm.yaml
    interval: 86400
  googlecn:
    type: http
    behavior: classical
    url: https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/GoogleCN.list
    path: ./ruleset/googlecn.yaml
    interval: 86400
  steamcn:
    type: http
    behavior: classical
    url: https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/Ruleset/SteamCN.list
    path: ./ruleset/steamcn.yaml
    interval: 86400
  bing:
    type: http
    behavior: classical
    url: https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/Bing.list
    path: ./ruleset/bing.yaml
    interval: 86400
  onedrive:
    type: http
    behavior: classical
    url: https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/OneDrive.list
    path: ./ruleset/onedrive.yaml
    interval: 86400
  microsoft:
    type: http
    behavior: classical
    url: https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/Microsoft.list
    path: ./ruleset/microsoft.yaml
    interval: 86400
  apple:
    type: http
    behavior: classical
    url: https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/Apple.list
    path: ./ruleset/apple.yaml
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
  - RULE-SET,banprogramad,应用净化
  - RULE-SET,googlefcm,全球直连
  - RULE-SET,googlecn,全球直连
  - RULE-SET,steamcn,全球直连
  - RULE-SET,bing,微软服务
  - RULE-SET,onedrive,微软服务
  - RULE-SET,microsoft,微软服务
  - RULE-SET,apple,苹果服务
  - RULE-SET,telegram,电报消息
  - RULE-SET,ai,Ai平台
  - RULE-SET,openai,Ai平台
  - RULE-SET,youtube,油管视频
  - RULE-SET,netflix,奈飞视频
  - RULE-SET,proxygfw,节点选择
  - RULE-SET,chinadomain,全球直连
  - RULE-SET,download,全球直连
  - GEOIP,CN,全球直连
```

The generated exporter will append the real `proxies`, an `AUTO` group, and `MATCH,AUTO`. If you prefer ACL4SSR's `漏网之鱼` group to be the final fallback, add `- MATCH,漏网之鱼` at the end of the template and disable or ignore the generated `MATCH,AUTO` fallback in a future exporter option.
