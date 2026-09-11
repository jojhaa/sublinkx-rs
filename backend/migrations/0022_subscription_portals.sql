ALTER TABLE subscriptions ADD COLUMN portal_enabled INTEGER NOT NULL DEFAULT 0;
ALTER TABLE subscriptions ADD COLUMN portal_slug TEXT;
ALTER TABLE subscriptions ADD COLUMN portal_access_code_hash TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_subscriptions_portal_slug
ON subscriptions(portal_slug);
