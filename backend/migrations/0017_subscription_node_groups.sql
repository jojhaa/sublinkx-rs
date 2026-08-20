CREATE TABLE IF NOT EXISTS subscription_node_groups (
  subscription_id INTEGER NOT NULL,
  node_group_id INTEGER NOT NULL,
  sort_order INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (subscription_id, node_group_id),
  FOREIGN KEY(subscription_id) REFERENCES subscriptions(id) ON DELETE CASCADE,
  FOREIGN KEY(node_group_id) REFERENCES node_groups(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_subscription_node_groups_sort
ON subscription_node_groups(subscription_id, sort_order);
