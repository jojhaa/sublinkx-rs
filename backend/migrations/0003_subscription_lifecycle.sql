ALTER TABLE subscriptions ADD COLUMN expires_at TEXT;

CREATE INDEX IF NOT EXISTS idx_subscriptions_expires_at ON subscriptions(expires_at);
