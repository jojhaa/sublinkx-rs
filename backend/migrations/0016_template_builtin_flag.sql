ALTER TABLE templates
ADD COLUMN is_builtin INTEGER NOT NULL DEFAULT 0;

UPDATE templates
SET is_builtin = 1
WHERE name IN (
  'Built-in Common Notes',
  'Built-in Clash ACL4SSR Style',
  'Built-in Mihomo Rule Base',
  'Built-in Xray URI Bundle',
  'Built-in Surge 4/5 Managed',
  'Built-in sing-box Route Base',
  'Built-in Surge 3 Managed',
  'Built-in Surge 2 Managed',
  'Built-in Quantumult X Base',
  'Built-in Quantumult Base',
  'Built-in Loon Base',
  'Built-in Surfboard Base',
  'Built-in Mellow Base',
  'Built-in ClashR Base',
  'Built-in Shadowsocks SIP002 Notes',
  'Built-in Shadowsocks SIP008 Base',
  'Built-in ShadowsocksR Notes',
  'Built-in ShadowsocksD Base',
  'Built-in Trojan URI Notes',
  'Built-in Mixed URI Notes'
);
