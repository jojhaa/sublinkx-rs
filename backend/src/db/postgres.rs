use sqlx::Executor;

use super::{DbPool, query};

pub(super) async fn init_schema(pool: &DbPool) -> Result<(), sqlx::Error> {
    let statements = [
        r#"
        CREATE TABLE IF NOT EXISTS users (
          id BIGSERIAL PRIMARY KEY,
          username VARCHAR(191) NOT NULL UNIQUE,
          password_hash VARCHAR(255) NOT NULL,
          nickname VARCHAR(191) NOT NULL DEFAULT '',
          role VARCHAR(64) NOT NULL DEFAULT 'admin',
          status VARCHAR(64) NOT NULL DEFAULT 'active',
          must_change_credentials BIGINT NOT NULL DEFAULT 1,
          token_version BIGINT NOT NULL DEFAULT 0,
          created_at VARCHAR(64) NOT NULL,
          updated_at VARCHAR(64) NOT NULL
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS node_groups (
          id BIGSERIAL PRIMARY KEY,
          name VARCHAR(191) NOT NULL UNIQUE,
          sort_order BIGINT NOT NULL DEFAULT 0,
          created_at VARCHAR(64) NOT NULL,
          updated_at VARCHAR(64) NOT NULL
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS nodes (
          id BIGSERIAL PRIMARY KEY,
          name VARCHAR(255) NOT NULL,
          protocol VARCHAR(64) NOT NULL,
          raw_link TEXT NOT NULL,
          server VARCHAR(255) NOT NULL,
          port BIGINT NOT NULL,
          enabled BIGINT NOT NULL DEFAULT 1,
          group_id BIGINT NULL,
          source_type VARCHAR(64) NOT NULL DEFAULT 'manual',
          source_ref TEXT NULL,
          upstream_missing BIGINT NOT NULL DEFAULT 0,
          fingerprint VARCHAR(191) NOT NULL,
          fingerprint_scope BIGINT NOT NULL DEFAULT 0,
          settings_json TEXT NOT NULL,
          remark TEXT NOT NULL,
          last_latency_ms BIGINT NULL,
          last_latency_status VARCHAR(64) NULL,
          last_latency_message TEXT NULL,
          last_latency_tested_at VARCHAR(64) NULL,
          created_at VARCHAR(64) NOT NULL,
          updated_at VARCHAR(64) NOT NULL,
          CONSTRAINT fk_nodes_group_id
            FOREIGN KEY (group_id) REFERENCES node_groups(id),
          CONSTRAINT uq_nodes_fingerprint_scope
            UNIQUE (fingerprint, fingerprint_scope)
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS templates (
          id BIGSERIAL PRIMARY KEY,
          name VARCHAR(191) NOT NULL UNIQUE,
          kind VARCHAR(64) NOT NULL,
          content TEXT NOT NULL,
          is_builtin BIGINT NOT NULL DEFAULT 0,
          created_at VARCHAR(64) NOT NULL,
          updated_at VARCHAR(64) NOT NULL
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS subscription_groups (
          id BIGSERIAL PRIMARY KEY,
          name VARCHAR(191) NOT NULL UNIQUE,
          sort_order BIGINT NOT NULL DEFAULT 0,
          created_at VARCHAR(64) NOT NULL,
          updated_at VARCHAR(64) NOT NULL
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS subscriptions (
          id BIGSERIAL PRIMARY KEY,
          name VARCHAR(191) NOT NULL UNIQUE,
          token VARCHAR(191) NOT NULL UNIQUE,
          description TEXT NOT NULL,
          default_client VARCHAR(64) NULL,
          template_id BIGINT NULL,
          group_id BIGINT NULL,
          enabled BIGINT NOT NULL DEFAULT 1,
          expires_at VARCHAR(64) NULL,
          portal_enabled BIGINT NOT NULL DEFAULT 0,
          portal_slug VARCHAR(191) NULL,
          portal_access_code_hash VARCHAR(255) NULL,
          created_at VARCHAR(64) NOT NULL,
          updated_at VARCHAR(64) NOT NULL,
          CONSTRAINT fk_subscriptions_template_id
            FOREIGN KEY (template_id) REFERENCES templates(id),
          CONSTRAINT fk_subscriptions_group_id
            FOREIGN KEY (group_id) REFERENCES subscription_groups(id)
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS subscription_nodes (
          subscription_id BIGINT NOT NULL,
          node_id BIGINT NOT NULL,
          sort_order BIGINT NOT NULL DEFAULT 0,
          PRIMARY KEY (subscription_id, node_id),
          CONSTRAINT fk_subscription_nodes_subscription_id
            FOREIGN KEY (subscription_id) REFERENCES subscriptions(id) ON DELETE CASCADE,
          CONSTRAINT fk_subscription_nodes_node_id
            FOREIGN KEY (node_id) REFERENCES nodes(id) ON DELETE CASCADE
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS subscription_node_groups (
          subscription_id BIGINT NOT NULL,
          node_group_id BIGINT NOT NULL,
          sort_order BIGINT NOT NULL DEFAULT 0,
          PRIMARY KEY (subscription_id, node_group_id),
          CONSTRAINT fk_subscription_node_groups_subscription_id
            FOREIGN KEY (subscription_id) REFERENCES subscriptions(id) ON DELETE CASCADE,
          CONSTRAINT fk_subscription_node_groups_node_group_id
            FOREIGN KEY (node_group_id) REFERENCES node_groups(id) ON DELETE CASCADE
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS access_logs (
          id BIGSERIAL PRIMARY KEY,
          subscription_id BIGINT NOT NULL,
          client_type VARCHAR(64) NULL,
          ip VARCHAR(128) NOT NULL,
          user_agent TEXT NOT NULL,
          status VARCHAR(64) NOT NULL,
          requested_at VARCHAR(64) NOT NULL,
          CONSTRAINT fk_access_logs_subscription_id
            FOREIGN KEY (subscription_id) REFERENCES subscriptions(id)
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS app_settings (
          "key" VARCHAR(191) PRIMARY KEY,
          value TEXT NOT NULL,
          updated_at VARCHAR(64) NOT NULL
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS upstream_subscriptions (
          id BIGSERIAL PRIMARY KEY,
          name VARCHAR(191) NOT NULL,
          url TEXT NOT NULL,
          group_id BIGINT NULL,
          enabled BIGINT NOT NULL DEFAULT 1,
          sync_enabled BIGINT NOT NULL DEFAULT 0,
          sync_interval_minutes BIGINT NOT NULL DEFAULT 360,
          remark TEXT NOT NULL DEFAULT '',
          last_imported_at VARCHAR(64) NULL,
          last_import_status VARCHAR(64) NULL,
          last_import_message TEXT NULL,
          last_import_imported BIGINT NOT NULL DEFAULT 0,
          last_import_updated BIGINT NOT NULL DEFAULT 0,
          last_import_disabled BIGINT NOT NULL DEFAULT 0,
          last_import_skipped BIGINT NOT NULL DEFAULT 0,
          last_import_failed BIGINT NOT NULL DEFAULT 0,
          template_id BIGINT NULL,
          template_name VARCHAR(191) NULL,
          created_at VARCHAR(64) NOT NULL,
          updated_at VARCHAR(64) NOT NULL,
          CONSTRAINT fk_upstream_subscriptions_group_id
            FOREIGN KEY (group_id) REFERENCES node_groups(id) ON DELETE SET NULL,
          CONSTRAINT fk_upstream_subscriptions_template_id
            FOREIGN KEY (template_id) REFERENCES templates(id) ON DELETE SET NULL
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS node_ip_probes (
          node_id BIGINT PRIMARY KEY,
          status VARCHAR(64) NOT NULL,
          ip VARCHAR(64) NULL,
          ip_version BIGINT NULL,
          exit_ip_revision BIGINT NOT NULL DEFAULT 0,
          country_code VARCHAR(8) NULL,
          country_name VARCHAR(128) NULL,
          country_source VARCHAR(64) NULL,
          intelligence_status VARCHAR(64) NULL,
          intelligence_message TEXT NULL,
          risk_ip VARCHAR(64) NULL,
          risk_status VARCHAR(64) NULL,
          scamalytics_fraud_score BIGINT NULL,
          scamalytics_isp_risk_score BIGINT NULL,
          risk_checked_at VARCHAR(64) NULL,
          risk_expires_at_unix_ms BIGINT NULL,
          risk_message TEXT NULL,
          risk_traits_json TEXT NULL,
          risk_traits_expires_at_unix_ms BIGINT NULL,
          message TEXT NULL,
          probed_at VARCHAR(64) NOT NULL,
          country_updated_at VARCHAR(64) NULL,
          intelligence_updated_at VARCHAR(64) NULL,
          updated_at VARCHAR(64) NOT NULL,
          CONSTRAINT fk_node_ip_probes_node_id
            FOREIGN KEY (node_id) REFERENCES nodes(id) ON DELETE CASCADE
        )
        "#,
        "ALTER TABLE subscriptions ADD COLUMN IF NOT EXISTS include_rules BIGINT NOT NULL DEFAULT 0",
        "ALTER TABLE subscriptions ADD COLUMN IF NOT EXISTS portal_enabled BIGINT NOT NULL DEFAULT 0",
        "ALTER TABLE subscriptions ADD COLUMN IF NOT EXISTS portal_slug VARCHAR(191) NULL",
        "ALTER TABLE subscriptions ADD COLUMN IF NOT EXISTS portal_access_code_hash VARCHAR(255) NULL",
        "ALTER TABLE node_ip_probes ADD COLUMN IF NOT EXISTS exit_ip_revision BIGINT NOT NULL DEFAULT 0",
        "ALTER TABLE node_ip_probes ADD COLUMN IF NOT EXISTS risk_ip VARCHAR(64) NULL",
        "ALTER TABLE node_ip_probes ADD COLUMN IF NOT EXISTS risk_status VARCHAR(64) NULL",
        "ALTER TABLE node_ip_probes ADD COLUMN IF NOT EXISTS scamalytics_fraud_score BIGINT NULL",
        "ALTER TABLE node_ip_probes ADD COLUMN IF NOT EXISTS scamalytics_isp_risk_score BIGINT NULL",
        "ALTER TABLE node_ip_probes ADD COLUMN IF NOT EXISTS risk_checked_at VARCHAR(64) NULL",
        "ALTER TABLE node_ip_probes ADD COLUMN IF NOT EXISTS risk_expires_at_unix_ms BIGINT NULL",
        "ALTER TABLE node_ip_probes ADD COLUMN IF NOT EXISTS risk_message TEXT NULL",
        "ALTER TABLE node_ip_probes ADD COLUMN IF NOT EXISTS risk_traits_json TEXT NULL",
        "ALTER TABLE node_ip_probes ADD COLUMN IF NOT EXISTS risk_traits_expires_at_unix_ms BIGINT NULL",
        "CREATE INDEX IF NOT EXISTS idx_nodes_protocol ON nodes(protocol)",
        "CREATE INDEX IF NOT EXISTS idx_nodes_group_id ON nodes(group_id)",
        "CREATE INDEX IF NOT EXISTS idx_nodes_last_latency_status ON nodes(last_latency_status)",
        "CREATE INDEX IF NOT EXISTS idx_subscriptions_group_id ON subscriptions(group_id)",
        "CREATE INDEX IF NOT EXISTS idx_subscriptions_expires_at ON subscriptions(expires_at)",
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_subscriptions_portal_slug ON subscriptions(portal_slug)",
        "CREATE INDEX IF NOT EXISTS idx_subscription_nodes_sort ON subscription_nodes(subscription_id, sort_order)",
        "CREATE INDEX IF NOT EXISTS idx_subscription_node_groups_sort ON subscription_node_groups(subscription_id, sort_order)",
        "CREATE INDEX IF NOT EXISTS idx_upstream_subscriptions_url ON upstream_subscriptions USING HASH (url)",
        "CREATE INDEX IF NOT EXISTS idx_upstream_subscriptions_group_id ON upstream_subscriptions(group_id)",
        "CREATE INDEX IF NOT EXISTS idx_upstream_subscriptions_enabled ON upstream_subscriptions(enabled)",
        "CREATE INDEX IF NOT EXISTS idx_upstream_subscriptions_sync ON upstream_subscriptions(enabled, sync_enabled)",
        "CREATE INDEX IF NOT EXISTS idx_node_ip_probes_country_code ON node_ip_probes(country_code)",
        "CREATE INDEX IF NOT EXISTS idx_node_ip_probes_intelligence_status ON node_ip_probes(intelligence_status)",
        "CREATE INDEX IF NOT EXISTS idx_node_ip_probes_risk_status ON node_ip_probes(risk_status)",
        "CREATE INDEX IF NOT EXISTS idx_node_ip_probes_status ON node_ip_probes(status)",
    ];

    for statement in statements {
        pool.execute(statement).await?;
    }

    let now = crate::utils::time::now_rfc3339();
    for (key, value) in [
        ("site.public_base_url", ""),
        ("latency.auto_enabled", "true"),
        ("latency.interval_minutes", "30"),
        ("latency.concurrency", "2"),
        ("latency.core_path", ""),
        ("latency.test_url", "https://cp.cloudflare.com/generate_204"),
        ("latency.timeout_secs", "10"),
    ] {
        query(
            r#"
            INSERT INTO app_settings ("key", value, updated_at)
            VALUES (?, ?, ?)
            ON CONFLICT ("key") DO NOTHING
            "#,
        )
        .bind(key)
        .bind(value)
        .bind(&now)
        .execute(pool)
        .await?;
    }

    Ok(())
}
