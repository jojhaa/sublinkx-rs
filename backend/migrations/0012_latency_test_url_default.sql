UPDATE app_settings
SET value = 'https://cp.cloudflare.com/generate_204',
    updated_at = datetime('now')
WHERE key = 'latency.test_url'
  AND value = 'https://www.gstatic.com/generate_204';

UPDATE templates
SET content = REPLACE(
        content,
        'https://www.gstatic.com/generate_204',
        'https://cp.cloudflare.com/generate_204'
    ),
    updated_at = datetime('now')
WHERE name IN ('Built-in Clash ACL4SSR Style', 'Built-in Mihomo Rule Base')
  AND content LIKE '%https://www.gstatic.com/generate_204%';

UPDATE templates
SET content = REPLACE(
        REPLACE(
            REPLACE(
                content,
                '  fallback:
    - https://1.1.1.1/dns-query
    - https://8.8.8.8/dns-query
  fallback-filter:
    geoip: true
    geoip-code: CN
',
                ''
            ),
            '  - GEOIP,CN,DIRECT
',
            ''
        ),
        '  - GEOIP,CN,全球直连
',
        ''
    ),
    updated_at = datetime('now')
WHERE name IN (
        'Built-in Clash ACL4SSR Style',
        'Built-in Mihomo Rule Base',
        'Built-in Mellow Base',
        'Built-in ClashR Base'
    )
  AND (
        content LIKE '%GEOIP,CN%'
        OR content LIKE '%fallback-filter:%'
    );
