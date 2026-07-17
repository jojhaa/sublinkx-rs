use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::OnceLock,
    time::Duration,
};

use sqlx::{
    AnyPool, Executor,
    any::{AnyConnectOptions, AnyPoolOptions, install_default_drivers},
};

#[allow(dead_code)]
pub const MIGRATIONS_DIR: &str = "migrations";

pub type DbPool = AnyPool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbKind {
    Sqlite,
    MySql,
}

static DB_KIND: OnceLock<DbKind> = OnceLock::new();

pub async fn new_database_pool(database_url: &str) -> Result<DbPool, sqlx::Error> {
    install_default_drivers();
    let kind = detect_db_kind(database_url);
    let _ = DB_KIND.set(kind);

    if kind == DbKind::Sqlite
        && let Some(path) = sqlite_file_path(database_url)
    {
        ensure_parent_dir(&path);
        ensure_sqlite_file(&path)?;
    }

    let options: AnyConnectOptions = database_url.parse()?;
    let pool = AnyPoolOptions::new()
        .max_connections(match kind {
            DbKind::Sqlite => 5,
            DbKind::MySql => 10,
        })
        .acquire_timeout(Duration::from_secs(10))
        .connect_with(options)
        .await?;

    match kind {
        DbKind::Sqlite => {
            configure_sqlite(&pool).await?;
            sqlx::migrate!("./migrations").run(&pool).await?;
            apply_builtin_template_content_upgrades(&pool).await?;
        }
        DbKind::MySql => {
            init_mysql_schema(&pool).await?;
        }
    }

    Ok(pool)
}

pub fn db_kind() -> DbKind {
    *DB_KIND.get().unwrap_or(&DbKind::Sqlite)
}

async fn configure_sqlite(pool: &DbPool) -> Result<(), sqlx::Error> {
    pool.execute("PRAGMA foreign_keys = ON").await?;
    pool.execute("PRAGMA journal_mode = WAL").await?;
    pool.execute("PRAGMA synchronous = NORMAL").await?;
    pool.execute("PRAGMA busy_timeout = 10000").await?;
    Ok(())
}

async fn init_mysql_schema(pool: &DbPool) -> Result<(), sqlx::Error> {
    let statements = [
        r#"
        CREATE TABLE IF NOT EXISTS users (
          id BIGINT PRIMARY KEY AUTO_INCREMENT,
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
          id BIGINT PRIMARY KEY AUTO_INCREMENT,
          name VARCHAR(191) NOT NULL UNIQUE,
          sort_order BIGINT NOT NULL DEFAULT 0,
          created_at VARCHAR(64) NOT NULL,
          updated_at VARCHAR(64) NOT NULL
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS nodes (
          id BIGINT PRIMARY KEY AUTO_INCREMENT,
          name VARCHAR(255) NOT NULL,
          protocol VARCHAR(64) NOT NULL,
          raw_link VARCHAR(2048) NOT NULL,
          server VARCHAR(255) NOT NULL,
          port BIGINT NOT NULL,
          enabled BOOLEAN NOT NULL DEFAULT TRUE,
          group_id BIGINT NULL,
          source_type VARCHAR(64) NOT NULL DEFAULT 'manual',
          source_ref VARCHAR(2048) NULL,
          upstream_missing BOOLEAN NOT NULL DEFAULT FALSE,
          fingerprint VARCHAR(191) NOT NULL,
          fingerprint_scope BIGINT NOT NULL DEFAULT 0,
          settings_json VARCHAR(4096) NOT NULL,
          remark VARCHAR(1024) NOT NULL,
          last_latency_ms BIGINT NULL,
          last_latency_status VARCHAR(64) NULL,
          last_latency_message TEXT NULL,
          last_latency_tested_at VARCHAR(64) NULL,
          created_at VARCHAR(64) NOT NULL,
          updated_at VARCHAR(64) NOT NULL,
          UNIQUE KEY idx_nodes_fingerprint_scope (fingerprint, fingerprint_scope),
          KEY idx_nodes_protocol (protocol),
          KEY idx_nodes_group_id (group_id),
          KEY idx_nodes_last_latency_status (last_latency_status),
          CONSTRAINT fk_nodes_group_id FOREIGN KEY (group_id) REFERENCES node_groups(id)
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS templates (
          id BIGINT PRIMARY KEY AUTO_INCREMENT,
          name VARCHAR(191) NOT NULL UNIQUE,
          kind VARCHAR(64) NOT NULL,
          content VARCHAR(12000) NOT NULL,
          created_at VARCHAR(64) NOT NULL,
          updated_at VARCHAR(64) NOT NULL
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS subscription_groups (
          id BIGINT PRIMARY KEY AUTO_INCREMENT,
          name VARCHAR(191) NOT NULL UNIQUE,
          sort_order BIGINT NOT NULL DEFAULT 0,
          created_at VARCHAR(64) NOT NULL,
          updated_at VARCHAR(64) NOT NULL
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS subscriptions (
          id BIGINT PRIMARY KEY AUTO_INCREMENT,
          name VARCHAR(191) NOT NULL UNIQUE,
          token VARCHAR(191) NOT NULL UNIQUE,
          description VARCHAR(1024) NOT NULL,
          default_client VARCHAR(64) NULL,
          template_id BIGINT NULL,
          group_id BIGINT NULL,
          enabled BOOLEAN NOT NULL DEFAULT TRUE,
          expires_at VARCHAR(64) NULL,
          created_at VARCHAR(64) NOT NULL,
          updated_at VARCHAR(64) NOT NULL,
          KEY idx_subscriptions_group_id (group_id),
          KEY idx_subscriptions_expires_at (expires_at),
          CONSTRAINT fk_subscriptions_template_id FOREIGN KEY (template_id) REFERENCES templates(id),
          CONSTRAINT fk_subscriptions_group_id FOREIGN KEY (group_id) REFERENCES subscription_groups(id)
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS subscription_nodes (
          subscription_id BIGINT NOT NULL,
          node_id BIGINT NOT NULL,
          sort_order BIGINT NOT NULL DEFAULT 0,
          PRIMARY KEY (subscription_id, node_id),
          KEY idx_subscription_nodes_sort (subscription_id, sort_order),
          CONSTRAINT fk_subscription_nodes_subscription_id FOREIGN KEY (subscription_id) REFERENCES subscriptions(id) ON DELETE CASCADE,
          CONSTRAINT fk_subscription_nodes_node_id FOREIGN KEY (node_id) REFERENCES nodes(id) ON DELETE CASCADE
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS access_logs (
          id BIGINT PRIMARY KEY AUTO_INCREMENT,
          subscription_id BIGINT NOT NULL,
          client_type VARCHAR(64) NULL,
          ip VARCHAR(128) NOT NULL,
          user_agent VARCHAR(2048) NOT NULL,
          status VARCHAR(64) NOT NULL,
          requested_at VARCHAR(64) NOT NULL,
          CONSTRAINT fk_access_logs_subscription_id FOREIGN KEY (subscription_id) REFERENCES subscriptions(id)
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS app_settings (
          `key` VARCHAR(191) PRIMARY KEY,
          value VARCHAR(2048) NOT NULL,
          updated_at VARCHAR(64) NOT NULL
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS upstream_subscriptions (
          id BIGINT PRIMARY KEY AUTO_INCREMENT,
          name VARCHAR(191) NOT NULL,
          url VARCHAR(2048) NOT NULL,
          group_id BIGINT NULL,
          enabled BOOLEAN NOT NULL DEFAULT TRUE,
          sync_enabled BOOLEAN NOT NULL DEFAULT FALSE,
          sync_interval_minutes BIGINT NOT NULL DEFAULT 360,
          remark VARCHAR(1024) NOT NULL DEFAULT '',
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
          KEY idx_upstream_subscriptions_url (url(191)),
          KEY idx_upstream_subscriptions_group_id (group_id),
          KEY idx_upstream_subscriptions_enabled (enabled),
          KEY idx_upstream_subscriptions_sync (enabled, sync_enabled),
          CONSTRAINT fk_upstream_subscriptions_group_id FOREIGN KEY (group_id) REFERENCES node_groups(id) ON DELETE SET NULL,
          CONSTRAINT fk_upstream_subscriptions_template_id FOREIGN KEY (template_id) REFERENCES templates(id) ON DELETE SET NULL
        )
        "#,
    ];

    for statement in statements {
        pool.execute(statement).await?;
    }

    apply_mysql_schema_upgrades(pool).await?;
    apply_builtin_template_content_upgrades(pool).await?;

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
        sqlx::query(
            r#"
            INSERT IGNORE INTO app_settings (`key`, value, updated_at)
            VALUES (?, ?, ?)
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

async fn apply_builtin_template_content_upgrades(pool: &DbPool) -> Result<(), sqlx::Error> {
    let now = crate::utils::time::now_rfc3339();
    for (name, kind, content, old_match) in [
        (
            "Built-in Clash ACL4SSR Style",
            "clash",
            crate::services::template_seed_service::CLASH_TEMPLATE,
            "%MATCH,节点选择%",
        ),
        (
            "Built-in Mihomo Rule Base",
            "mihomo",
            crate::services::template_seed_service::MIHOMO_TEMPLATE,
            "%MATCH,PROXY%",
        ),
    ] {
        sqlx::query(
            r#"
            UPDATE templates
            SET content = ?, updated_at = ?
            WHERE name = ?
              AND kind = ?
              AND content NOT LIKE '%rule-providers:%'
              AND content LIKE ?
            "#,
        )
        .bind(content)
        .bind(&now)
        .bind(name)
        .bind(kind)
        .bind(old_match)
        .execute(pool)
        .await?;
    }

    Ok(())
}

async fn apply_mysql_schema_upgrades(pool: &DbPool) -> Result<(), sqlx::Error> {
    let now = crate::utils::time::now_rfc3339();

    for (table, column, definition) in [
        (
            "users",
            "must_change_credentials",
            "BIGINT NOT NULL DEFAULT 1",
        ),
        ("users", "token_version", "BIGINT NOT NULL DEFAULT 0"),
        ("nodes", "group_id", "BIGINT NULL"),
        (
            "nodes",
            "source_type",
            "VARCHAR(64) NOT NULL DEFAULT 'manual'",
        ),
        ("nodes", "source_ref", "VARCHAR(2048) NULL"),
        (
            "nodes",
            "upstream_missing",
            "BOOLEAN NOT NULL DEFAULT FALSE",
        ),
        ("nodes", "fingerprint_scope", "BIGINT NOT NULL DEFAULT 0"),
        ("nodes", "last_latency_ms", "BIGINT NULL"),
        ("nodes", "last_latency_status", "VARCHAR(64) NULL"),
        ("nodes", "last_latency_message", "TEXT NULL"),
        ("nodes", "last_latency_tested_at", "VARCHAR(64) NULL"),
        ("subscriptions", "group_id", "BIGINT NULL"),
        ("subscriptions", "expires_at", "VARCHAR(64) NULL"),
        (
            "upstream_subscriptions",
            "sync_enabled",
            "BOOLEAN NOT NULL DEFAULT FALSE",
        ),
        (
            "upstream_subscriptions",
            "sync_interval_minutes",
            "BIGINT NOT NULL DEFAULT 360",
        ),
        (
            "upstream_subscriptions",
            "last_import_updated",
            "BIGINT NOT NULL DEFAULT 0",
        ),
        (
            "upstream_subscriptions",
            "last_import_disabled",
            "BIGINT NOT NULL DEFAULT 0",
        ),
    ] {
        mysql_add_column_if_missing(pool, table, column, definition).await?;
    }

    if mysql_index_exists(pool, "nodes", "idx_nodes_fingerprint").await? {
        pool.execute("DROP INDEX idx_nodes_fingerprint ON nodes")
            .await?;
    }
    pool.execute("UPDATE nodes SET fingerprint_scope = COALESCE(group_id, 0)")
        .await?;

    for (table, index, create_sql) in [
        (
            "nodes",
            "idx_nodes_fingerprint_scope",
            "CREATE UNIQUE INDEX idx_nodes_fingerprint_scope ON nodes(fingerprint, fingerprint_scope)",
        ),
        (
            "nodes",
            "idx_nodes_protocol",
            "CREATE INDEX idx_nodes_protocol ON nodes(protocol)",
        ),
        (
            "nodes",
            "idx_nodes_group_id",
            "CREATE INDEX idx_nodes_group_id ON nodes(group_id)",
        ),
        (
            "nodes",
            "idx_nodes_last_latency_status",
            "CREATE INDEX idx_nodes_last_latency_status ON nodes(last_latency_status)",
        ),
        (
            "subscriptions",
            "idx_subscriptions_group_id",
            "CREATE INDEX idx_subscriptions_group_id ON subscriptions(group_id)",
        ),
        (
            "subscriptions",
            "idx_subscriptions_expires_at",
            "CREATE INDEX idx_subscriptions_expires_at ON subscriptions(expires_at)",
        ),
        (
            "upstream_subscriptions",
            "idx_upstream_subscriptions_sync",
            "CREATE INDEX idx_upstream_subscriptions_sync ON upstream_subscriptions(enabled, sync_enabled)",
        ),
        (
            "subscription_nodes",
            "idx_subscription_nodes_sort",
            "CREATE INDEX idx_subscription_nodes_sort ON subscription_nodes(subscription_id, sort_order)",
        ),
    ] {
        mysql_add_index_if_missing(pool, table, index, create_sql).await?;
    }

    pool.execute(
        r#"
        UPDATE app_settings
        SET value = 'https://cp.cloudflare.com/generate_204'
        WHERE `key` = 'latency.test_url'
          AND value = 'https://www.gstatic.com/generate_204'
        "#,
    )
    .await?;
    pool.execute(
        r#"
        UPDATE templates
        SET content = REPLACE(
                content,
                'https://www.gstatic.com/generate_204',
                'https://cp.cloudflare.com/generate_204'
            )
        WHERE name IN ('Built-in Clash ACL4SSR Style', 'Built-in Mihomo Rule Base')
          AND content LIKE '%https://www.gstatic.com/generate_204%'
        "#,
    )
    .await?;
    pool.execute(
        r#"
        UPDATE templates
        SET content = REPLACE(
                REPLACE(
                    REPLACE(
                        content,
                        '  fallback:
    - https://1.1.1.1/dns-query
    - https://8.8.8.8/dns-query
  fallback-filter:
    geoip: true
    geoip-code: CN
',
                        ''
                    ),
                    '  - GEOIP,CN,DIRECT
',
                    ''
                ),
                '  - GEOIP,CN,全球直连
',
                ''
            )
        WHERE name IN (
                'Built-in Clash ACL4SSR Style',
                'Built-in Mihomo Rule Base',
                'Built-in Mellow Base',
                'Built-in ClashR Base'
            )
          AND (
                content LIKE '%GEOIP,CN%'
                OR content LIKE '%fallback-filter:%'
            )
        "#,
    )
    .await?;
    sqlx::query(
        r#"
        UPDATE templates
        SET content = REPLACE(
                content,
                '      - 自动选择
      - 手动切换
',
                '      - 手动切换
      - 自动选择
'
            ),
            updated_at = ?
        WHERE name = 'Built-in Clash ACL4SSR Style'
          AND content LIKE '%      - 自动选择
      - 手动切换
%'
        "#,
    )
    .bind(&now)
    .execute(pool)
    .await?;
    sqlx::query(
        r#"
        UPDATE templates
        SET content = REPLACE(
                content,
                '      - AUTO
      - MANUAL
',
                '      - MANUAL
      - AUTO
'
            ),
            updated_at = ?
        WHERE name = 'Built-in Mihomo Rule Base'
          AND content LIKE '%      - AUTO
      - MANUAL
%'
        "#,
    )
    .bind(&now)
    .execute(pool)
    .await?;
    sqlx::query(
        r#"
        UPDATE templates
        SET content = REPLACE(
                content,
                '  - RULE-SET,googlecn,全球直连
',
                '  - DOMAIN-SUFFIX,chatgpt.com,Ai平台
  - DOMAIN-SUFFIX,openai.com,Ai平台
  - DOMAIN-SUFFIX,anthropic.com,Ai平台
  - DOMAIN-SUFFIX,claude.ai,Ai平台
  - DOMAIN-SUFFIX,github.com,节点选择
  - DOMAIN-SUFFIX,githubusercontent.com,节点选择
  - DOMAIN-SUFFIX,githubassets.com,节点选择
  - DOMAIN-SUFFIX,youtube.com,油管视频
  - DOMAIN-SUFFIX,googlevideo.com,油管视频
  - DOMAIN-SUFFIX,ytimg.com,油管视频
  - DOMAIN-SUFFIX,netflix.com,奈飞视频
  - DOMAIN-SUFFIX,nflxvideo.net,奈飞视频
  - DOMAIN-SUFFIX,t.me,电报消息
  - DOMAIN-SUFFIX,telegram.org,电报消息
  - DOMAIN-SUFFIX,x.com,节点选择
  - DOMAIN-SUFFIX,twitter.com,节点选择
  - DOMAIN-SUFFIX,instagram.com,节点选择
  - DOMAIN-SUFFIX,tiktok.com,节点选择
  - DOMAIN-SUFFIX,spotify.com,节点选择
  - DOMAIN-SUFFIX,disneyplus.com,节点选择
  - DOMAIN-SUFFIX,primevideo.com,节点选择
  - DOMAIN-SUFFIX,max.com,节点选择
  - DOMAIN-SUFFIX,hbomax.com,节点选择
  - DOMAIN-SUFFIX,steamcommunity.com,节点选择
  - DOMAIN-SUFFIX,steampowered.com,节点选择
  - DOMAIN-SUFFIX,epicgames.com,节点选择
  - RULE-SET,googlecn,全球直连
'
            ),
            updated_at = ?
        WHERE name = 'Built-in Clash ACL4SSR Style'
          AND content NOT LIKE '%DOMAIN-SUFFIX,chatgpt.com%'
          AND content LIKE '%  - RULE-SET,googlecn,全球直连%'
        "#,
    )
    .bind(&now)
    .execute(pool)
    .await?;
    sqlx::query(
        r#"
        UPDATE templates
        SET content = REPLACE(
                content,
                '  - RULE-SET,ai,AI
',
                '  - DOMAIN-SUFFIX,chatgpt.com,AI
  - DOMAIN-SUFFIX,openai.com,AI
  - DOMAIN-SUFFIX,anthropic.com,AI
  - DOMAIN-SUFFIX,claude.ai,AI
  - DOMAIN-SUFFIX,github.com,PROXY
  - DOMAIN-SUFFIX,githubusercontent.com,PROXY
  - DOMAIN-SUFFIX,githubassets.com,PROXY
  - DOMAIN-SUFFIX,youtube.com,YOUTUBE
  - DOMAIN-SUFFIX,googlevideo.com,YOUTUBE
  - DOMAIN-SUFFIX,ytimg.com,YOUTUBE
  - DOMAIN-SUFFIX,netflix.com,NETFLIX
  - DOMAIN-SUFFIX,nflxvideo.net,NETFLIX
  - DOMAIN-SUFFIX,t.me,TELEGRAM
  - DOMAIN-SUFFIX,telegram.org,TELEGRAM
  - DOMAIN-SUFFIX,x.com,MEDIA
  - DOMAIN-SUFFIX,twitter.com,MEDIA
  - DOMAIN-SUFFIX,instagram.com,MEDIA
  - DOMAIN-SUFFIX,tiktok.com,MEDIA
  - DOMAIN-SUFFIX,spotify.com,MEDIA
  - DOMAIN-SUFFIX,disneyplus.com,MEDIA
  - DOMAIN-SUFFIX,primevideo.com,MEDIA
  - DOMAIN-SUFFIX,max.com,MEDIA
  - DOMAIN-SUFFIX,hbomax.com,MEDIA
  - DOMAIN-SUFFIX,steamcommunity.com,PROXY
  - DOMAIN-SUFFIX,steampowered.com,PROXY
  - DOMAIN-SUFFIX,epicgames.com,PROXY
  - RULE-SET,ai,AI
'
            ),
            updated_at = ?
        WHERE name = 'Built-in Mihomo Rule Base'
          AND content NOT LIKE '%DOMAIN-SUFFIX,chatgpt.com%'
          AND content LIKE '%  - RULE-SET,ai,AI%'
        "#,
    )
    .bind(&now)
    .execute(pool)
    .await?;

    Ok(())
}

async fn mysql_add_column_if_missing(
    pool: &DbPool,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<(), sqlx::Error> {
    if mysql_column_exists(pool, table, column).await? {
        return Ok(());
    }

    let sql = format!("ALTER TABLE `{table}` ADD COLUMN `{column}` {definition}");
    pool.execute(sql.as_str()).await?;
    Ok(())
}

async fn mysql_add_index_if_missing(
    pool: &DbPool,
    table: &str,
    index: &str,
    create_sql: &str,
) -> Result<(), sqlx::Error> {
    if mysql_index_exists(pool, table, index).await? {
        return Ok(());
    }

    pool.execute(create_sql).await?;
    Ok(())
}

async fn mysql_column_exists(
    pool: &DbPool,
    table: &str,
    column: &str,
) -> Result<bool, sqlx::Error> {
    let count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM information_schema.columns
        WHERE table_schema = DATABASE()
          AND table_name = ?
          AND column_name = ?
        "#,
    )
    .bind(table)
    .bind(column)
    .fetch_one(pool)
    .await?;

    Ok(count > 0)
}

async fn mysql_index_exists(pool: &DbPool, table: &str, index: &str) -> Result<bool, sqlx::Error> {
    let count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM information_schema.statistics
        WHERE table_schema = DATABASE()
          AND table_name = ?
          AND index_name = ?
        "#,
    )
    .bind(table)
    .bind(index)
    .fetch_one(pool)
    .await?;

    Ok(count > 0)
}

fn detect_db_kind(database_url: &str) -> DbKind {
    if database_url.starts_with("mysql://") || database_url.starts_with("mariadb://") {
        DbKind::MySql
    } else {
        DbKind::Sqlite
    }
}

fn ensure_parent_dir(path: &Path) {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        let _ = fs::create_dir_all(parent);
    }
}

fn ensure_sqlite_file(path: &Path) -> Result<(), sqlx::Error> {
    if path.exists() {
        return Ok(());
    }
    fs::File::create(path).map(|_| ()).map_err(sqlx::Error::Io)
}

fn sqlite_file_path(database_url: &str) -> Option<PathBuf> {
    if !database_url.starts_with("sqlite:") {
        return None;
    }

    if database_url.contains(":memory:") {
        return None;
    }

    let path_part = database_url
        .trim_start_matches("sqlite://")
        .trim_start_matches("sqlite:");

    if path_part.is_empty() {
        return None;
    }

    let candidate = PathBuf::from(path_part);
    if candidate.is_absolute() {
        return Some(candidate);
    }

    env::current_dir().ok().map(|cwd| cwd.join(candidate))
}
