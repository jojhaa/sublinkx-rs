# Clash 分流模板说明

Clash/Mihomo 的分流配置主要由两部分组成：

- `proxy-groups`：策略组，用于决定最终使用哪个出站节点。
- `rules`：按顺序匹配的规则，把流量分配到策略组、`DIRECT` 或 `REJECT`。

在 `sublinkx-rs` 中，导出器会把真实节点注入到 `proxies`，然后追加一个 `AUTO` 策略组和最终的 `MATCH,AUTO` 兜底规则。Clash 模板可以在兜底规则之前定义额外的策略组、规则集、DNS 和分流规则。

## 最小分流模板

创建一个 `kind = clash` 的模板，并粘贴下面的 YAML：

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

## 匹配机制

规则会从上到下依次匹配，命中第一条后停止。因此广告拦截、私有网络、直连规则应放在宽泛代理规则之前。导出器会在模板规则后追加 `MATCH,AUTO`，所以未匹配流量仍有兜底策略。

`rules` 会把流量送到 `PROXY`、`AI`、`STREAMING` 等策略组。`proxy-groups` 决定这些策略组如何选择节点。生成的 `AUTO` 组包含当前导出的节点，可以在模板中安全引用。

## 实用策略组模式

建议把 `PROXY` 作为通用代理组，再创建 `AI`、`STREAMING`、`GAME`、`DOWNLOAD` 等业务组。规则可以把特定域名导向业务组，其余流量由 `AUTO` 兜底。

如果某些 Clash 客户端不支持远程 `rule-providers`，可以把 `RULE-SET` 替换成直接规则，例如：

```yaml
rules:
  - DOMAIN-SUFFIX,openai.com,AI
  - DOMAIN-SUFFIX,google.com,PROXY
  - GEOIP,CN,DIRECT
```

## ACL4SSR 风格完整分流模板

`youshandefeiyang/sub-web-modify` 不直接写死 Clash YAML，而是把远程 subconverter `.ini` 配置传给后端。它的默认 Clash 分流主要参考 ACL4SSR 远程配置，例如：

- `ACL4SSR_Online_Full_NoAuto.ini`
- `ACL4SSR_Online_Full.ini`
- `GeneralClashRule.ini`

这种风格有两个核心点：

- 大量 `ruleset=PolicyGroup,RuleListUrl` 把流量导入命名策略组。
- 大量 `custom_proxy_group=Name\`type\`filter` 创建手动选择、自动测速、故障转移、地区、媒体、AI 和兜底策略组。

本项目使用原生 Clash/Mihomo YAML 模板，而不是 subconverter ini。最接近的原生模板如下：

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

导出器会追加真实 `proxies`、`AUTO` 组和 `MATCH,AUTO`。如果你希望 ACL4SSR 风格的 `漏网之鱼` 作为最终兜底，可以在模板末尾加入 `- MATCH,漏网之鱼`。后续导出器可以增加选项，用于禁用或忽略自动生成的 `MATCH,AUTO` 兜底。
