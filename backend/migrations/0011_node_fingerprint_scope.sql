DROP INDEX IF EXISTS idx_nodes_fingerprint;

ALTER TABLE nodes ADD COLUMN fingerprint_scope INTEGER NOT NULL DEFAULT 0;

UPDATE nodes
SET fingerprint_scope = COALESCE(group_id, 0);

CREATE UNIQUE INDEX IF NOT EXISTS idx_nodes_fingerprint_scope
ON nodes(fingerprint, fingerprint_scope);
