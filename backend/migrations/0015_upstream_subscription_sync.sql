ALTER TABLE upstream_subscriptions
ADD COLUMN sync_enabled INTEGER NOT NULL DEFAULT 0;

ALTER TABLE upstream_subscriptions
ADD COLUMN sync_interval_minutes INTEGER NOT NULL DEFAULT 360;

ALTER TABLE upstream_subscriptions
ADD COLUMN last_import_updated INTEGER NOT NULL DEFAULT 0;

ALTER TABLE upstream_subscriptions
ADD COLUMN last_import_disabled INTEGER NOT NULL DEFAULT 0;

ALTER TABLE nodes
ADD COLUMN upstream_missing INTEGER NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_upstream_subscriptions_sync
ON upstream_subscriptions(enabled, sync_enabled);
