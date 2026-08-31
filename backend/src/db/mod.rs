use std::{
    collections::HashMap,
    env,
    fmt::Display,
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::Duration,
};

use sqlx::{
    Any, AnyPool, Arguments, Encode, Executor, FromRow, Type,
    any::{AnyArguments, AnyConnectOptions, AnyPoolOptions, AnyRow, install_default_drivers},
};

mod postgres;

#[allow(dead_code)]
pub const MIGRATIONS_DIR: &str = "migrations";

pub type DbPool = AnyPool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbKind {
    Sqlite,
    MySql,
    Postgres,
}

static DB_KIND: OnceLock<DbKind> = OnceLock::new();
static POSTGRES_SQL_CACHE: OnceLock<Mutex<HashMap<&'static str, &'static str>>> = OnceLock::new();

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
            DbKind::Postgres => 10,
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
        DbKind::Postgres => {
            postgres::init_schema(&pool).await?;
            apply_builtin_template_content_upgrades(&pool).await?;
        }
    }

    Ok(pool)
}

pub(crate) async fn connect_database_pool_uninitialized(
    database_url: &str,
    create_sqlite_file: bool,
) -> Result<DbPool, sqlx::Error> {
    install_default_drivers();
    let kind = detect_db_kind(database_url);

    if kind == DbKind::Sqlite
        && let Some(path) = sqlite_file_path(database_url)
    {
        if create_sqlite_file {
            ensure_parent_dir(&path);
            ensure_sqlite_file(&path)?;
        } else if !path.exists() {
            return Err(sqlx::Error::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("source SQLite database does not exist: {}", path.display()),
            )));
        }
    }

    let options: AnyConnectOptions = database_url.parse()?;
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(10))
        .connect_with(options)
        .await?;

    if kind == DbKind::Sqlite {
        pool.execute("PRAGMA busy_timeout = 10000").await?;
    }

    Ok(pool)
}

pub fn db_kind() -> DbKind {
    *DB_KIND.get().unwrap_or(&DbKind::Sqlite)
}

/// Returns SQL adapted to the active backend. Application SQL uses `?` bind
/// markers and MySQL-style quoted identifiers; PostgreSQL needs `$N` markers
/// and ANSI identifier quotes.
pub fn database_sql(sql: &'static str) -> &'static str {
    if db_kind() != DbKind::Postgres {
        return sql;
    }

    let cache = POSTGRES_SQL_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut cache = cache.lock().expect("PostgreSQL SQL cache is poisoned");
    if let Some(adapted) = cache.get(sql) {
        return adapted;
    }

    let adapted = Box::leak(adapt_postgres_sql(sql).into_boxed_str());
    cache.insert(sql, adapted);
    adapted
}

pub fn database_sql_owned(sql: impl Into<String>) -> String {
    let sql = sql.into();
    if db_kind() == DbKind::Postgres {
        adapt_postgres_sql(&sql)
    } else {
        sql
    }
}

pub fn query<'query>(sql: &'static str) -> sqlx::query::Query<'query, Any, AnyArguments<'query>> {
    sqlx::query::<Any>(database_sql(sql))
}

pub fn query_as<'query, O>(
    sql: &'static str,
) -> sqlx::query::QueryAs<'query, Any, O, AnyArguments<'query>>
where
    O: for<'row> FromRow<'row, AnyRow>,
{
    sqlx::query_as::<Any, O>(database_sql(sql))
}

pub fn query_scalar<'query, O>(
    sql: &'static str,
) -> sqlx::query::QueryScalar<'query, Any, O, AnyArguments<'query>>
where
    (O,): for<'row> FromRow<'row, AnyRow>,
{
    sqlx::query_scalar::<Any, O>(database_sql(sql))
}

/// SQLx's `QueryBuilder<Any>` always emits `?`, even when the runtime driver is
/// PostgreSQL. This small builder keeps AnyPool while adapting the final SQL.
pub struct DbQueryBuilder<'args> {
    sql: String,
    arguments: Option<AnyArguments<'args>>,
    prepared: bool,
}

impl<'args> DbQueryBuilder<'args> {
    pub fn new(sql: impl Into<String>) -> Self {
        Self {
            sql: sql.into(),
            arguments: Some(AnyArguments::default()),
            prepared: false,
        }
    }

    pub fn push(&mut self, sql: impl Display) -> &mut Self {
        assert!(!self.prepared, "query cannot be changed after build");
        use std::fmt::Write as _;
        write!(self.sql, "{sql}").expect("failed to append SQL");
        self
    }

    pub fn push_bind<T>(&mut self, value: T) -> &mut Self
    where
        T: 'args + Encode<'args, Any> + Type<Any>,
    {
        assert!(!self.prepared, "query cannot be changed after build");
        self.arguments
            .as_mut()
            .expect("query arguments were already consumed")
            .add(value)
            .expect("failed to add query argument");
        self.sql.push('?');
        self
    }

    pub fn separated<'builder>(
        &'builder mut self,
        separator: &'static str,
    ) -> DbSeparated<'builder, 'args> {
        DbSeparated {
            builder: self,
            separator,
            push_separator: false,
        }
    }

    pub fn build(&'args mut self) -> sqlx::query::Query<'args, Any, AnyArguments<'args>> {
        self.prepare();
        let arguments = self
            .arguments
            .take()
            .expect("query builder must not be reused after build");
        sqlx::query_with::<Any, _>(&self.sql, arguments)
    }

    pub fn build_query_as<O>(
        &'args mut self,
    ) -> sqlx::query::QueryAs<'args, Any, O, AnyArguments<'args>>
    where
        O: for<'row> FromRow<'row, AnyRow>,
    {
        self.prepare();
        let arguments = self
            .arguments
            .take()
            .expect("query builder must not be reused after build");
        sqlx::query_as_with::<Any, O, _>(&self.sql, arguments)
    }

    pub fn build_query_scalar<O>(
        &'args mut self,
    ) -> sqlx::query::QueryScalar<'args, Any, O, AnyArguments<'args>>
    where
        (O,): for<'row> FromRow<'row, AnyRow>,
    {
        self.prepare();
        let arguments = self
            .arguments
            .take()
            .expect("query builder must not be reused after build");
        sqlx::query_scalar_with::<Any, O, _>(&self.sql, arguments)
    }

    fn prepare(&mut self) {
        if self.prepared {
            return;
        }
        if db_kind() == DbKind::Postgres {
            self.sql = adapt_postgres_sql(&self.sql);
        }
        self.prepared = true;
    }
}

pub struct DbSeparated<'builder, 'args> {
    builder: &'builder mut DbQueryBuilder<'args>,
    separator: &'static str,
    push_separator: bool,
}

impl<'builder, 'args> DbSeparated<'builder, 'args> {
    pub fn push_bind<T>(&mut self, value: T) -> &mut Self
    where
        T: 'args + Encode<'args, Any> + Type<Any>,
    {
        if self.push_separator {
            self.builder.push(self.separator);
        }
        self.builder.push_bind(value);
        self.push_separator = true;
        self
    }
}

fn adapt_postgres_sql(sql: &str) -> String {
    #[derive(Clone, Copy)]
    enum State {
        Normal,
        SingleQuote,
        DoubleQuote,
        Backtick,
        LineComment,
        BlockComment,
    }

    let mut output = String::with_capacity(sql.len() + 16);
    let mut chars = sql.chars().peekable();
    let mut state = State::Normal;
    let mut parameter = 0_u32;

    while let Some(ch) = chars.next() {
        match state {
            State::Normal => match ch {
                '\'' => {
                    output.push(ch);
                    state = State::SingleQuote;
                }
                '"' => {
                    output.push(ch);
                    state = State::DoubleQuote;
                }
                '`' => {
                    output.push('"');
                    state = State::Backtick;
                }
                '-' if chars.peek() == Some(&'-') => {
                    output.push(ch);
                    output.push(chars.next().expect("peeked SQL comment marker"));
                    state = State::LineComment;
                }
                '/' if chars.peek() == Some(&'*') => {
                    output.push(ch);
                    output.push(chars.next().expect("peeked SQL comment marker"));
                    state = State::BlockComment;
                }
                '?' => {
                    parameter += 1;
                    output.push('$');
                    output.push_str(&parameter.to_string());
                }
                _ => output.push(ch),
            },
            State::SingleQuote => {
                output.push(ch);
                if ch == '\'' {
                    if chars.peek() == Some(&'\'') {
                        output.push(chars.next().expect("peeked escaped quote"));
                    } else {
                        state = State::Normal;
                    }
                }
            }
            State::DoubleQuote => {
                output.push(ch);
                if ch == '"' {
                    if chars.peek() == Some(&'"') {
                        output.push(chars.next().expect("peeked escaped quote"));
                    } else {
                        state = State::Normal;
                    }
                }
            }
            State::Backtick => {
                if ch == '`' {
                    output.push('"');
                    state = State::Normal;
                } else {
                    output.push(ch);
                }
            }
            State::LineComment => {
                output.push(ch);
                if ch == '\n' {
                    state = State::Normal;
                }
            }
            State::BlockComment => {
                output.push(ch);
                if ch == '*' && chars.peek() == Some(&'/') {
                    output.push(chars.next().expect("peeked SQL comment terminator"));
                    state = State::Normal;
                }
            }
        }
    }

    output
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
          is_builtin BOOLEAN NOT NULL DEFAULT FALSE,
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
        CREATE TABLE IF NOT EXISTS subscription_node_groups (
          subscription_id BIGINT NOT NULL,
          node_group_id BIGINT NOT NULL,
          sort_order BIGINT NOT NULL DEFAULT 0,
          PRIMARY KEY (subscription_id, node_group_id),
          KEY idx_subscription_node_groups_sort (subscription_id, sort_order),
          CONSTRAINT fk_subscription_node_groups_subscription_id FOREIGN KEY (subscription_id) REFERENCES subscriptions(id) ON DELETE CASCADE,
          CONSTRAINT fk_subscription_node_groups_node_group_id FOREIGN KEY (node_group_id) REFERENCES node_groups(id) ON DELETE CASCADE
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
        r#"
        CREATE TABLE IF NOT EXISTS node_ip_probes (
          node_id BIGINT PRIMARY KEY,
          status VARCHAR(64) NOT NULL,
          ip VARCHAR(64) NULL,
          ip_version BIGINT NULL,
          country_code VARCHAR(8) NULL,
          country_name VARCHAR(128) NULL,
          country_source VARCHAR(64) NULL,
          intelligence_status VARCHAR(64) NULL,
          intelligence_message TEXT NULL,
          message TEXT NULL,
          probed_at VARCHAR(64) NOT NULL,
          country_updated_at VARCHAR(64) NULL,
          intelligence_updated_at VARCHAR(64) NULL,
          updated_at VARCHAR(64) NOT NULL,
          KEY idx_node_ip_probes_country_code (country_code),
          KEY idx_node_ip_probes_intelligence_status (intelligence_status),
          KEY idx_node_ip_probes_status (status),
          CONSTRAINT fk_node_ip_probes_node_id FOREIGN KEY (node_id) REFERENCES nodes(id) ON DELETE CASCADE
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
        query(
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
        query(
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

    query(
        r#"
        UPDATE templates
        SET content = REPLACE(
                REPLACE(
                    REPLACE(
                        content,
                        '  ipv6: true
',
                        '  ipv6: false
'
                    ),
                    '    - https://1.1.1.1/dns-query
    - https://8.8.8.8/dns-query
',
                    ''
                ),
                '  respect-rules: true
',
                '  respect-rules: false
'
            ),
            updated_at = ?
        WHERE name = 'Built-in Mihomo Policy Orchestrator'
          AND is_builtin <> 0
          AND (
                content LIKE '%  ipv6: true%'
                OR content LIKE '%  respect-rules: true%'
                OR content LIKE '%    - https://1.1.1.1/dns-query
    - https://8.8.8.8/dns-query%'
            )
        "#,
    )
    .bind(&now)
    .execute(pool)
    .await?;

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
        ("templates", "is_builtin", "BOOLEAN NOT NULL DEFAULT FALSE"),
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
        ("node_ip_probes", "intelligence_status", "VARCHAR(64) NULL"),
        ("node_ip_probes", "intelligence_message", "TEXT NULL"),
        (
            "node_ip_probes",
            "intelligence_updated_at",
            "VARCHAR(64) NULL",
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
    pool.execute(
        r#"
        UPDATE templates
        SET is_builtin = TRUE
        WHERE name IN (
            'Built-in Common Notes',
            'Built-in Clash ACL4SSR Style',
            'Built-in Mihomo Rule Base',
            'Built-in Mihomo Policy Orchestrator',
            'Built-in Xray URI Bundle',
            'Built-in Surge 4/5 Managed',
            'Built-in sing-box Route Base',
            'Built-in Surge 3 Managed',
            'Built-in Surge 2 Managed',
            'Built-in Quantumult X Base',
            'Built-in Quantumult Base',
            'Built-in Loon Base',
            'Built-in Surfboard Base',
            'Built-in Mellow Base',
            'Built-in ClashR Base',
            'Built-in Shadowsocks SIP002 Notes',
            'Built-in Shadowsocks SIP008 Base',
            'Built-in ShadowsocksR Notes',
            'Built-in ShadowsocksD Base',
            'Built-in Trojan URI Notes',
            'Built-in Mixed URI Notes'
        )
        "#,
    )
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
        (
            "node_ip_probes",
            "idx_node_ip_probes_intelligence_status",
            "CREATE INDEX idx_node_ip_probes_intelligence_status ON node_ip_probes(intelligence_status)",
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
    query(
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
    query(
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
    query(
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
    query(
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

pub(crate) fn detect_db_kind(database_url: &str) -> DbKind {
    if database_url.starts_with("mysql://") || database_url.starts_with("mariadb://") {
        DbKind::MySql
    } else if database_url.starts_with("postgres://") || database_url.starts_with("postgresql://") {
        DbKind::Postgres
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

pub(crate) fn sqlite_file_path(database_url: &str) -> Option<PathBuf> {
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

#[cfg(test)]
mod tests {
    use sqlx::{Executor, Row, any::AnyPoolOptions};

    use super::{
        DbKind, adapt_postgres_sql, apply_builtin_template_content_upgrades, detect_db_kind,
        new_database_pool,
    };

    const OLD_DNS: &str = "dns:\n  ipv6: true\n  proxy-server-nameserver:\n    - https://dns.alidns.com/dns-query\n    - https://doh.pub/dns-query\n    - https://1.1.1.1/dns-query\n    - https://8.8.8.8/dns-query\n  respect-rules: true\n";

    #[test]
    fn detects_postgres_urls_and_rewrites_only_sql_syntax() {
        assert_eq!(
            detect_db_kind("postgresql://user:password@localhost/sublinkx"),
            DbKind::Postgres
        );
        assert_eq!(
            detect_db_kind("postgres://user:password@localhost/sublinkx"),
            DbKind::Postgres
        );
        assert_eq!(detect_db_kind("mysql://localhost/sublinkx"), DbKind::MySql);
        assert_eq!(detect_db_kind("sqlite://data/app.db"), DbKind::Sqlite);

        assert_eq!(
            adapt_postgres_sql(
                "SELECT `key`, '?' AS literal FROM app_settings WHERE `key` = ? -- ?\nAND value = ? /* ? */"
            ),
            "SELECT \"key\", '?' AS literal FROM app_settings WHERE \"key\" = $1 -- ?\nAND value = $2 /* ? */"
        );
    }

    #[tokio::test]
    async fn postgres_repository_smoke_when_configured() {
        let Ok(database_url) = std::env::var("SUBLINKX_TEST_POSTGRES_URL") else {
            return;
        };

        use crate::{
            config::{
                AppConfig, DatabaseConfig, IpIntelligenceConfig, SecurityConfig, ServerConfig,
            },
            repository::{
                group_repo::{self, GroupTable, NewGroupRecord},
                node_ip_probe_repo,
                node_repo::{self, NewNodeRecord},
                settings_repo,
                subscription_repo::{self, NewSubscriptionRecord},
                template_repo::{self, NewTemplateRecord},
                upstream_subscription_repo::{self, NewUpstreamSubscriptionRecord},
                user_repo,
            },
            services::template_seed_service,
        };

        let pool = new_database_pool(&database_url)
            .await
            .expect("PostgreSQL schema should initialize");
        let config = AppConfig {
            server: ServerConfig {
                port: 0,
                environment: "test".to_string(),
            },
            database: DatabaseConfig {
                url: database_url.clone(),
            },
            security: SecurityConfig {
                jwt_secret: "postgres-smoke-secret".repeat(2),
                jwt_exp_hours: 24,
                bootstrap_admin_username: "postgres-admin".to_string(),
                bootstrap_admin_password: "postgres-smoke-password".to_string(),
                trust_proxy_headers: false,
                auth_cookie_secure: false,
            },
            ip_intelligence: IpIntelligenceConfig {
                enabled: false,
                base_url: String::new(),
                api_token: String::new(),
                source_key: "postgres-smoke".to_string(),
            },
        };
        user_repo::bootstrap_admin(&pool, &config)
            .await
            .expect("admin bootstrap should work on PostgreSQL");
        assert!(
            user_repo::find_by_username(&pool, "postgres-admin")
                .await
                .expect("admin lookup should work")
                .is_some()
        );

        template_seed_service::seed_default_templates(&pool)
            .await
            .expect("template seeding should work on PostgreSQL");
        assert!(
            template_repo::count_filtered(&pool, None)
                .await
                .expect("template count should work")
                > 0
        );

        settings_repo::set(&pool, "postgres.smoke", "one", "2026-08-30T00:00:00Z")
            .await
            .expect("setting insert should work");
        settings_repo::set(&pool, "postgres.smoke", "two", "2026-08-30T00:00:01Z")
            .await
            .expect("setting upsert should work");
        assert_eq!(
            settings_repo::get_many(&pool, &["postgres.smoke"])
                .await
                .expect("setting lookup should work")
                .get("postgres.smoke")
                .map(String::as_str),
            Some("two")
        );

        let now = "2026-08-30T00:00:00Z";
        let group = group_repo::insert(
            &pool,
            GroupTable::Node,
            &NewGroupRecord {
                name: "PostgreSQL smoke group",
                sort_order: 1,
                created_at: now,
                updated_at: now,
            },
        )
        .await
        .expect("group insert should work");
        let node = node_repo::insert(
            &pool,
            &NewNodeRecord {
                name: "PostgreSQL smoke node",
                protocol: "vless",
                raw_link: "vless://postgres-smoke@example.com:443",
                server: "example.com",
                port: 443,
                enabled: 1,
                group_id: Some(group.id),
                fingerprint_scope: group.id,
                source_type: "manual",
                source_ref: None,
                upstream_missing: 0,
                fingerprint: "postgres-smoke-node",
                settings_json: "{}",
                remark: "",
                created_at: now,
                updated_at: now,
            },
        )
        .await
        .expect("node insert should work");
        assert_eq!(
            node_repo::list_page(&pool, Some(group.id), false, Some(true), true, 10, 0)
                .await
                .expect("node pagination should work")
                .len(),
            1
        );

        let subscription = subscription_repo::insert_with_nodes(
            &pool,
            &NewSubscriptionRecord {
                name: "PostgreSQL smoke subscription",
                token: "postgres-smoke-token",
                description: "",
                default_client: Some("mihomo"),
                template_id: None,
                group_id: None,
                enabled: 1,
                expires_at: None,
                created_at: now,
                updated_at: now,
            },
            &[node.id],
            &[group.id],
        )
        .await
        .expect("subscription transaction should work");
        assert_eq!(
            subscription_repo::list_subscription_nodes(&pool, subscription.id)
                .await
                .expect("subscription nodes should load")
                .len(),
            1
        );

        let upstream = upstream_subscription_repo::insert(
            &pool,
            &NewUpstreamSubscriptionRecord {
                name: "PostgreSQL smoke upstream",
                url: "https://example.com/postgres-smoke",
                group_id: Some(group.id),
                enabled: 1,
                sync_enabled: 1,
                sync_interval_minutes: 60,
                remark: "",
                created_at: now,
                updated_at: now,
            },
        )
        .await
        .expect("upstream insert should work");
        assert_eq!(
            upstream_subscription_repo::list_page(&pool, 10, 0)
                .await
                .expect("upstream pagination should work")
                .len(),
            1
        );

        let upstream_template_name = "Upstream Mihomo PostgreSQL a1b2c3d4";
        let old_template = template_repo::insert(
            &pool,
            &NewTemplateRecord {
                name: upstream_template_name,
                kind: "mihomo",
                content: "x-sublinkx-upstream-template: true\nproxies: [old]\n",
                is_builtin: 0,
                created_at: now,
                updated_at: now,
            },
        )
        .await
        .expect("old upstream template should insert");
        let latest_template_name = format!("{upstream_template_name} #2");
        let latest_template = template_repo::insert(
            &pool,
            &NewTemplateRecord {
                name: &latest_template_name,
                kind: "mihomo",
                content: "x-sublinkx-upstream-template: true\nproxies: [newer]\n",
                is_builtin: 0,
                created_at: now,
                updated_at: now,
            },
        )
        .await
        .expect("latest upstream template should insert");
        crate::db::query("UPDATE subscriptions SET template_id = ? WHERE id = ?")
            .bind(old_template.id)
            .bind(subscription.id)
            .execute(&pool)
            .await
            .expect("subscription template should bind");
        crate::db::query(
            "UPDATE upstream_subscriptions SET template_id = ?, template_name = ? WHERE id = ?",
        )
        .bind(old_template.id)
        .bind(upstream_template_name)
        .bind(upstream.id)
        .execute(&pool)
        .await
        .expect("upstream template should bind");

        let consolidated = template_repo::upsert_latest_upstream_passthrough(
            &pool,
            "a1b2c3d4",
            &NewTemplateRecord {
                name: upstream_template_name,
                kind: "mihomo",
                content: "x-sublinkx-upstream-template: true\nproxies: [latest]\n",
                is_builtin: 0,
                created_at: now,
                updated_at: "2026-08-30T00:00:01Z",
            },
        )
        .await
        .expect("upstream templates should consolidate on PostgreSQL");
        assert_eq!(consolidated.template.id, latest_template.id);
        assert_eq!(consolidated.removed_templates, 1);
        assert!(
            template_repo::find_by_id(&pool, old_template.id)
                .await
                .expect("old template lookup should work")
                .is_none()
        );
        assert_eq!(
            subscription_repo::find_by_id(&pool, subscription.id)
                .await
                .expect("subscription lookup should work")
                .expect("subscription should exist")
                .template_id,
            Some(latest_template.id)
        );
        assert_eq!(
            upstream_subscription_repo::find_by_id(&pool, upstream.id)
                .await
                .expect("upstream lookup should work")
                .expect("upstream should exist")
                .template_id,
            Some(latest_template.id)
        );

        let probe = node_ip_probe_repo::save_success(&pool, node.id, "203.0.113.10", 4, now)
            .await
            .expect("IP probe upsert should work");
        assert_eq!(probe.ip.as_deref(), Some("203.0.113.10"));

        pool.close().await;
        let reopened = new_database_pool(&database_url)
            .await
            .expect("PostgreSQL schema initialization should be idempotent");
        assert_eq!(
            subscription_repo::count_filtered(&reopened, None, false)
                .await
                .expect("data should survive reconnect"),
            1
        );
        reopened.close().await;
    }

    #[tokio::test]
    async fn upgrades_only_the_builtin_mihomo_orchestrator_dns() {
        sqlx::any::install_default_drivers();
        let pool = AnyPoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        pool.execute(
            "CREATE TABLE templates (name TEXT NOT NULL, kind TEXT NOT NULL, content TEXT NOT NULL, is_builtin BOOLEAN NOT NULL, updated_at TEXT NOT NULL)",
        )
        .await
        .unwrap();
        for (name, is_builtin) in [
            ("Built-in Mihomo Policy Orchestrator", 1_i64),
            ("User Mihomo Policy Orchestrator", 0_i64),
        ] {
            sqlx::query(
                "INSERT INTO templates (name, kind, content, is_builtin, updated_at) VALUES (?, 'mihomo', ?, ?, 'before')",
            )
            .bind(name)
            .bind(OLD_DNS)
            .bind(is_builtin)
            .execute(&pool)
            .await
            .unwrap();
        }

        apply_builtin_template_content_upgrades(&pool)
            .await
            .unwrap();
        apply_builtin_template_content_upgrades(&pool)
            .await
            .unwrap();

        let rows = sqlx::query("SELECT name, content FROM templates ORDER BY name")
            .fetch_all(&pool)
            .await
            .unwrap();
        let builtin: String = rows[0].get("content");
        let custom: String = rows[1].get("content");
        assert!(
            builtin.contains("  ipv6: false\n"),
            "unexpected built-in content: {builtin:?}"
        );
        assert!(builtin.contains("  respect-rules: false\n"));
        assert!(!builtin.contains("https://1.1.1.1/dns-query\n"));
        assert!(!builtin.contains("https://8.8.8.8/dns-query\n"));
        assert_eq!(custom, OLD_DNS);
    }
}
