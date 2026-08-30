# Clash 分流模板说明

Clash/Mihomo 的分流配置主要由两部分组成：

- `proxy-groups`：策略组，用于决定最终使用哪个出站节点。
- `rules`：按顺序匹配的规则，把流量分配到策略组、`DIRECT` 或 `REJECT`。

在 `sublinkx-rs` 中，导出器会把真实节点注入到 `proxies`，并把 `include-all-proxies: true` 的策略组展开为当前订阅节点。模板可以定义额外的策略组、规则集、DNS 和分流规则；如果模板已有 `rules`，需要在模板里显式写最终 `MATCH` 兜底。

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

proxy-groups:
  - name: PROXY
    type: select
    proxies:
      - MANUAL
      - AUTO
      - DIRECT

  - name: MANUAL
    type: select
    include-all-proxies: true

  - name: AI
    type: select
    proxies:
      - PROXY
      - MANUAL
      - AUTO
      - DIRECT

  - name: STREAMING
    type: select
    proxies:
      - PROXY
      - MANUAL
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
```

## 匹配机制

规则会从上到下依次匹配，命中第一条后停止。因此广告拦截、私有网络、直连规则应放在宽泛代理规则之前。导出器会在模板规则后追加 `MATCH,AUTO`，所以未匹配流量仍有兜底策略。

`rules` 会把流量送到 `PROXY`、`AI`、`STREAMING` 等策略组。`proxy-groups` 决定这些策略组如何选择节点。生成的 `AUTO` 组包含当前导出的节点，可以在模板中安全引用。

普通 Mihomo 导出固定提供 `MANUAL`、`AUTO`、`FALLBACK` 和 `LOAD-BALANCE` 四种节点策略。启用 `ip-intelligence-rs` 且节点已有与当前出口 IP 一致的新鲜国家代码时，导出器还会按固定顺序生成“香港负载”“日本负载”“新加坡负载”“美国负载”“台湾负载”和“其他地区负载”。国家组使用 `load-balance` 与 `consistent-hashing`，并挂到 `PROXY` 或模板中的首个 `select` 主选择组；香港、日本、新加坡、美国和台湾之外的有效国家代码统一归入“其他地区负载”。国家未知的节点不会被排除，仍保留在四个全节点策略中；导出器不会根据节点名称猜测国家。

### 分组内自动回退

普通 Mihomo 导出按国家节点数量生成策略组：同国家至少两个节点才生成独立负载组，至少三个节点才同时生成故障转移组；香港、日本、新加坡、美国或台湾只有一个节点时，该节点进入“其他地区负载”。

- 国家范围使用“香港故障转移”“日本故障转移”等名称，只包含对应国家或其他地区的节点。
- 后台数据库节点分组只用于节点组织和导出排序，不生成客户端策略组，避免 Mihomo 面板出现大量“节点分组·…”卡片。
- 组内按节点名称稳定排序，使用 `fallback`、120 秒健康检查、5 秒超时、2 次失败阈值和 HTTP 204 预期状态。
- `lazy: true` 保证未选择的故障转移组不执行周期测试，避免所有分组同时产生探测流量。

需要自动回退时，应在 `PROXY` 中选择对应国家“故障转移”组。直接在 `MANUAL` 中固定单个原始节点仍保持手动固定语义，不会在后台悄悄切换；全局 `FALLBACK` 仍覆盖当前订阅全部节点。

少于三个订阅可用节点的国家不会生成故障转移组。未识别为香港、日本、新加坡、美国或台湾的国家继续进入“其他地区负载”，“其他地区”不生成跨国家故障转移。模板已有同名自定义组时导出器不会覆盖或自动引用它，避免把成员未知的用户配置误当成受控分组。

## 系统策略编排模板

新安装会获得 `Built-in Mihomo Policy Orchestrator` 系统模板。它在四种节点策略和六个国家负载组之外，提供 `AI`、`STREAMING`、`GOOGLE`、`TELEGRAM`、`GAME`、`DOMESTIC` 与 `FINAL` 网站分流组。系统以新增模板的方式交付，不批量覆盖数据库中的旧模板或用户自定义模板；已有订阅需要主动选择新模板后才会使用新规则。

### DNS 分流

系统编排模板使用以下 DNS 层级：

- `rule-set:private_domain` 和 `rule-set:cn_domain` 使用阿里、腾讯 DoH，处理私有和国内域名。
- `rule-set:geolocation-not-cn` 以及未命中专用策略的域名使用 Cloudflare、Google DoH，并通过 `PROXY` 策略组发起查询，减少国外域名的本地 DNS 泄漏。
- `proxy-server-nameserver` 仅解析代理节点服务器域名，使用可直连的国内 DoH，避免境外直连 DoH 被阻断或重置时产生间歇性 TLS 错误。
- 全局 `respect-rules` 保持关闭；需要境外 DNS 的查询由对应服务器上的显式 `#PROXY` 负责，避免 DNS 连接再次经过整套路由规则形成启动依赖。
- DNS 默认关闭 IPv6，避免无稳定 IPv6 出口的设备获得不可达结果；确认客户端和代理链路均支持 IPv6 后，可以在自定义模板中开启。
- `direct-nameserver` 处理最终走 `DIRECT` 的域名，并继续遵循 `nameserver-policy`。
- `fake-ip-filter` 排除局域网、本地域名、系统时间、网络连通性检测和常见门户探测域名，这些域名返回真实 IP。

固定域名解析使用顶层 `hosts`。系统默认保持空映射，避免替用户绑定任何生产域名；需要时可在自定义模板中填写：

```yaml
hosts:
  fixed.example.com: 192.0.2.10
  dual-stack.example.com:
    - 192.0.2.20
    - 2001:db8::20
```

示例使用保留域名和文档网段，不能直接作为生产记录。固定地址变化后必须同步更新模板，否则会绕过正常 DNS 获得过期地址。

模板支持 Mihomo 原生规则表达式。用户自定义模板可以直接编写域名、IP、目标端口、TCP/UDP 网络类型、进程名称和逻辑组合，例如：

```yaml
rules:
  - DOMAIN-SUFFIX,example.com,PROXY
  - IP-CIDR,198.51.100.0/24,DIRECT,no-resolve
  - DST-PORT,8443,PROXY
  - NETWORK,udp,PROXY
  - PROCESS-NAME,example-client,PROXY
  - AND,((DOMAIN,example.com),(NETWORK,tcp)),PROXY
  - OR,((DST-PORT,80),(DST-PORT,443)),PROXY
  - NOT,((DOMAIN,blocked.example)),PROXY
  - MATCH,FINAL
```

导出器只补齐缺失的系统节点组，不改写模板已有 `rules`，因此规则顺序和 `AND/OR/NOT` 组合会原样保留。

## BT 与大流量下载边界

系统编排模板默认把常见 BT 客户端进程、Tracker 规则集和典型 BT 端口送入 `DOWNLOAD-BLOCK`，该组默认选择 `REJECT`，管理员可在客户端临时切换为 `DIRECT`。进程规则依赖客户端平台和 Mihomo 运行模式提供进程识别能力；随机端口、加密或私有 Tracker 可能绕过端口与域名规则。

Mihomo 路由规则匹配连接属性，不能按连接累计字节数判定“普通大文件下载”。因此浏览器或其他程序通过正常 HTTPS 下载大文件时，系统模板不会仅凭文件大小阻断；若需要按流量阈值控制，必须另行增加流量计量、配额和强制断流模块。

## 实用策略组模式

建议把 `PROXY` 作为通用代理组，再创建 `AI`、`STREAMING`、`GAME`、`DOWNLOAD` 等业务组。规则可以把特定域名导向业务组，其余流量由 `AUTO` 兜底。

如果某些 Clash 客户端不支持远程 `rule-providers`，可以把 `RULE-SET` 替换成直接规则，例如：

```yaml
rules:
  - DOMAIN-SUFFIX,openai.com,AI
  - DOMAIN-SUFFIX,google.com,PROXY
  - RULE-SET,direct,DIRECT
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

proxy-groups:
  - name: 节点选择
    type: select
    proxies:
      - 手动切换
      - 自动选择
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
    url: https://cp.cloudflare.com/generate_204
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
    url: https://cp.cloudflare.com/generate_204
    interval: 300
    tolerance: 50

  - name: 日本节点
    type: url-test
    include-all-proxies: true
    filter: "(?i)日本|东京|大阪|jp|japan"
    url: https://cp.cloudflare.com/generate_204
    interval: 300
    tolerance: 50

  - name: 美国节点
    type: url-test
    include-all-proxies: true
    filter: "(?i)美|us|united states|america|los angeles|san jose|seattle"
    url: https://cp.cloudflare.com/generate_204
    interval: 300
    tolerance: 150

  - name: 狮城节点
    type: url-test
    include-all-proxies: true
    filter: "(?i)新加坡|狮城|sg|singapore"
    url: https://cp.cloudflare.com/generate_204
    interval: 300
    tolerance: 50

  - name: 台湾节点
    type: url-test
    include-all-proxies: true
    filter: "(?i)台|tw|taiwan"
    url: https://cp.cloudflare.com/generate_204
    interval: 300
    tolerance: 50

  - name: 韩国节点
    type: url-test
    include-all-proxies: true
    filter: "(?i)韩|kr|korea|seoul"
    url: https://cp.cloudflare.com/generate_204
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
  - DOMAIN-SUFFIX,x.com,国外媒体
  - DOMAIN-SUFFIX,twitter.com,国外媒体
  - DOMAIN-SUFFIX,instagram.com,国外媒体
  - DOMAIN-SUFFIX,tiktok.com,国外媒体
  - DOMAIN-SUFFIX,spotify.com,国外媒体
  - DOMAIN-SUFFIX,disneyplus.com,国外媒体
  - DOMAIN-SUFFIX,primevideo.com,国外媒体
  - DOMAIN-SUFFIX,max.com,国外媒体
  - DOMAIN-SUFFIX,hbomax.com,国外媒体
  - DOMAIN-SUFFIX,steamcommunity.com,游戏平台
  - DOMAIN-SUFFIX,steampowered.com,游戏平台
  - DOMAIN-SUFFIX,epicgames.com,游戏平台
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
  - MATCH,漏网之鱼
```

导出器会追加真实 `proxies`，并把 `include-all-proxies: true` 的策略组展开为当前订阅节点。如果模板已有 `rules`，导出器会保留模板规则，不再强行追加 `MATCH,AUTO`，因此完整模板末尾应保留 `- MATCH,漏网之鱼`。
