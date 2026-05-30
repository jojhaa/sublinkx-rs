ALTER TABLE nodes ADD COLUMN last_latency_ms INTEGER;
ALTER TABLE nodes ADD COLUMN last_latency_status TEXT;
ALTER TABLE nodes ADD COLUMN last_latency_message TEXT;
ALTER TABLE nodes ADD COLUMN last_latency_tested_at TEXT;

CREATE INDEX IF NOT EXISTS idx_nodes_last_latency_status ON nodes(last_latency_status);
