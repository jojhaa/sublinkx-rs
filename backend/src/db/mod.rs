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
          fingerprint VARCHAR(191) NOT NULL,
          settings_json VARCHAR(4096) NOT NULL,
          remark VARCHAR(1024) NOT NULL,
          last_latency_ms BIGINT NULL,
          last_latency_status VARCHAR(64) NULL,
          last_latency_message TEXT NULL,
          last_latency_tested_at VARCHAR(64) NULL,
          created_at VARCHAR(64) NOT NULL,
          updated_at VARCHAR(64) NOT NULL,
          UNIQUE KEY idx_nodes_fingerprint (fingerprint),
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
    ];

    for statement in statements {
        pool.execute(statement).await?;
    }

    let now = crate::utils::time::now_rfc3339();
    for (key, value) in [
        ("site.public_base_url", ""),
        ("latency.auto_enabled", "true"),
        ("latency.interval_minutes", "30"),
        ("latency.core_path", ""),
        ("latency.test_url", "https://www.gstatic.com/generate_204"),
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
