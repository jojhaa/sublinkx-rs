CREATE TABLE IF NOT EXISTS node_ip_probes (
  node_id INTEGER PRIMARY KEY,
  status TEXT NOT NULL,
  ip TEXT,
  ip_version INTEGER,
  country_code TEXT,
  country_name TEXT,
  country_source TEXT,
  message TEXT,
  probed_at TEXT NOT NULL,
  country_updated_at TEXT,
  updated_at TEXT NOT NULL,
  FOREIGN KEY(node_id) REFERENCES nodes(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_node_ip_probes_country_code
ON node_ip_probes(country_code);

CREATE INDEX IF NOT EXISTS idx_node_ip_probes_status
ON node_ip_probes(status);
