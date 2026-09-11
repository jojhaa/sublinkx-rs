ALTER TABLE node_ip_probes ADD COLUMN exit_ip_revision INTEGER NOT NULL DEFAULT 0;
ALTER TABLE node_ip_probes ADD COLUMN risk_ip TEXT;
ALTER TABLE node_ip_probes ADD COLUMN risk_status TEXT;
ALTER TABLE node_ip_probes ADD COLUMN scamalytics_fraud_score INTEGER;
ALTER TABLE node_ip_probes ADD COLUMN scamalytics_isp_risk_score INTEGER;
ALTER TABLE node_ip_probes ADD COLUMN risk_checked_at TEXT;
ALTER TABLE node_ip_probes ADD COLUMN risk_expires_at_unix_ms INTEGER;
ALTER TABLE node_ip_probes ADD COLUMN risk_message TEXT;

CREATE INDEX IF NOT EXISTS idx_node_ip_probes_risk_status
ON node_ip_probes(risk_status);
