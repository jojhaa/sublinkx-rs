CREATE TABLE IF NOT EXISTS upstream_subscriptions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  url TEXT NOT NULL UNIQUE,
  group_id INTEGER NULL,
  enabled INTEGER NOT NULL DEFAULT 1,
  remark TEXT NOT NULL DEFAULT '',
  last_imported_at TEXT NULL,
  last_import_status TEXT NULL,
  last_import_message TEXT NULL,
  last_import_imported INTEGER NOT NULL DEFAULT 0,
  last_import_skipped INTEGER NOT NULL DEFAULT 0,
  last_import_failed INTEGER NOT NULL DEFAULT 0,
  template_id INTEGER NULL,
  template_name TEXT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY(group_id) REFERENCES node_groups(id) ON DELETE SET NULL,
  FOREIGN KEY(template_id) REFERENCES templates(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_upstream_subscriptions_group_id
ON upstream_subscriptions(group_id);

CREATE INDEX IF NOT EXISTS idx_upstream_subscriptions_enabled
ON upstream_subscriptions(enabled);
