ALTER TABLE node_ip_probes ADD COLUMN intelligence_status TEXT;
ALTER TABLE node_ip_probes ADD COLUMN intelligence_message TEXT;
ALTER TABLE node_ip_probes ADD COLUMN intelligence_updated_at TEXT;

CREATE INDEX IF NOT EXISTS idx_node_ip_probes_intelligence_status
ON node_ip_probes(intelligence_status);
