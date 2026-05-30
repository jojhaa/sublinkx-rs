INSERT OR IGNORE INTO app_settings (key, value, updated_at)
VALUES
  ('latency.core_path', '', datetime('now')),
  ('latency.test_url', 'https://www.gstatic.com/generate_204', datetime('now')),
  ('latency.timeout_secs', '10', datetime('now'));
