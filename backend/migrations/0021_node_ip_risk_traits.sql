ALTER TABLE node_ip_probes ADD COLUMN risk_traits_json TEXT;
ALTER TABLE node_ip_probes ADD COLUMN risk_traits_expires_at_unix_ms INTEGER;
