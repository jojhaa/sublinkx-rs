CREATE TABLE IF NOT EXISTS subscription_groups (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL UNIQUE,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

ALTER TABLE subscriptions ADD COLUMN group_id INTEGER;

CREATE INDEX IF NOT EXISTS idx_subscriptions_group_id ON subscriptions(group_id);
