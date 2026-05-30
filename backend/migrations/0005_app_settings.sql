CREATE TABLE IF NOT EXISTS app_settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

INSERT OR IGNORE INTO app_settings (key, value, updated_at)
VALUES
  ('latency.auto_enabled', 'true', datetime('now')),
  ('latency.interval_minutes', '30', datetime('now'));
