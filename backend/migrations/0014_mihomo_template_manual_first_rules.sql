UPDATE templates
SET content = REPLACE(
        content,
        '      - 自动选择
      - 手动切换
',
        '      - 手动切换
      - 自动选择
'
    ),
    updated_at = datetime('now')
WHERE name = 'Built-in Clash ACL4SSR Style'
  AND content LIKE '%      - 自动选择
      - 手动切换
%';

UPDATE templates
SET content = REPLACE(
        content,
        '      - AUTO
      - MANUAL
',
        '      - MANUAL
      - AUTO
'
    ),
    updated_at = datetime('now')
WHERE name = 'Built-in Mihomo Rule Base'
  AND content LIKE '%      - AUTO
      - MANUAL
%';

UPDATE templates
SET content = REPLACE(
        content,
        '  - RULE-SET,googlecn,全球直连
',
        '  - DOMAIN-SUFFIX,chatgpt.com,Ai平台
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
'
    ),
    updated_at = datetime('now')
WHERE name = 'Built-in Clash ACL4SSR Style'
  AND content NOT LIKE '%DOMAIN-SUFFIX,chatgpt.com%'
  AND content LIKE '%  - RULE-SET,googlecn,全球直连%';

UPDATE templates
SET content = REPLACE(
        content,
        '  - RULE-SET,ai,AI
',
        '  - DOMAIN-SUFFIX,chatgpt.com,AI
  - DOMAIN-SUFFIX,openai.com,AI
  - DOMAIN-SUFFIX,anthropic.com,AI
  - DOMAIN-SUFFIX,claude.ai,AI
  - DOMAIN-SUFFIX,github.com,PROXY
  - DOMAIN-SUFFIX,githubusercontent.com,PROXY
  - DOMAIN-SUFFIX,githubassets.com,PROXY
  - DOMAIN-SUFFIX,youtube.com,YOUTUBE
  - DOMAIN-SUFFIX,googlevideo.com,YOUTUBE
  - DOMAIN-SUFFIX,ytimg.com,YOUTUBE
  - DOMAIN-SUFFIX,netflix.com,NETFLIX
  - DOMAIN-SUFFIX,nflxvideo.net,NETFLIX
  - DOMAIN-SUFFIX,t.me,TELEGRAM
  - DOMAIN-SUFFIX,telegram.org,TELEGRAM
  - DOMAIN-SUFFIX,x.com,MEDIA
  - DOMAIN-SUFFIX,twitter.com,MEDIA
  - DOMAIN-SUFFIX,instagram.com,MEDIA
  - DOMAIN-SUFFIX,tiktok.com,MEDIA
  - DOMAIN-SUFFIX,spotify.com,MEDIA
  - DOMAIN-SUFFIX,disneyplus.com,MEDIA
  - DOMAIN-SUFFIX,primevideo.com,MEDIA
  - DOMAIN-SUFFIX,max.com,MEDIA
  - DOMAIN-SUFFIX,hbomax.com,MEDIA
  - DOMAIN-SUFFIX,steamcommunity.com,PROXY
  - DOMAIN-SUFFIX,steampowered.com,PROXY
  - DOMAIN-SUFFIX,epicgames.com,PROXY
  - RULE-SET,ai,AI
'
    ),
    updated_at = datetime('now')
WHERE name = 'Built-in Mihomo Rule Base'
  AND content NOT LIKE '%DOMAIN-SUFFIX,chatgpt.com%'
  AND content LIKE '%  - RULE-SET,ai,AI%';
