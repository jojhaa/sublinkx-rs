use std::{collections::BTreeMap, env};

use sqlx::{Any, Row, Transaction, any::AnyRow};
use thiserror::Error;

use crate::db::{
    DbKind, DbPool, connect_database_pool_uninitialized, detect_db_kind, new_database_pool,
    sqlite_file_path,
};

const SOURCE_URL_ENV: &str = "SUBLINKX_MIGRATION_SOURCE_URL";
const TARGET_URL_ENV: &str = "SUBLINKX_MIGRATION_TARGET_URL";
const CONFIRM_ENV: &str = "SUBLINKX_MIGRATION_CONFIRM";
const REQUIRED_CONFIRMATION: &str = "I_HAVE_A_CURRENT_BACKUP";
const BATCH_SIZE: i64 = 500;
const INITIAL_APP_SETTINGS: &[(&str, &str)] = &[
    ("site.public_base_url", ""),
    ("latency.auto_enabled", "true"),
    ("latency.interval_minutes", "30"),
    ("latency.concurrency", "2"),
    ("latency.core_path", ""),
    ("latency.test_url", "https://cp.cloudflare.com/generate_204"),
    ("latency.timeout_secs", "10"),
];

#[derive(Debug, Error)]
pub enum DatabaseMigrationError {
    #[error("missing or empty environment variable {0}")]
    MissingEnvironment(&'static str),
    #[error(
        "unsupported database URL in {variable}; expected sqlite://, mysql://, mariadb://, postgres://, or postgresql://"
    )]
    UnsupportedDatabaseUrl { variable: &'static str },
    #[error("source and target database URLs must be different")]
    SameDatabase,
    #[error("source database is missing required tables: {0}")]
    MissingSourceTables(String),
    #[error("target database is not empty: {0}; migrate into a new empty database")]
    TargetNotEmpty(String),
    #[error(
        "migration execution requires {CONFIRM_ENV}={REQUIRED_CONFIRMATION}; run check first and create a current backup"
    )]
    ConfirmationRequired,
    #[error(
        "unsupported migrate-database arguments; use `migrate-database check` or `migrate-database run`"
    )]
    InvalidArguments,
    #[error("{operation} failed for table {table}: {source}")]
    TableOperation {
        operation: &'static str,
        table: &'static str,
        #[source]
        source: sqlx::Error,
    },
    #[error("row count mismatch for table {table}: source={source_count}, target={target_count}")]
    CountMismatch {
        table: &'static str,
        source_count: i64,
        target_count: i64,
    },
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

#[derive(Debug, Clone)]
struct MigrationConfig {
    source_url: String,
    target_url: String,
    source_kind: DbKind,
    target_kind: DbKind,
}

#[derive(Debug)]
struct DatabaseInspection {
    counts: BTreeMap<&'static str, i64>,
    missing_tables: Vec<&'static str>,
}

#[derive(Debug)]
struct MigrationReport {
    source_kind: DbKind,
    target_kind: DbKind,
    counts: BTreeMap<&'static str, i64>,
}

#[derive(Clone, Copy)]
enum MigrationMode {
    Check,
    Run,
}

#[derive(Clone, Copy)]
enum ValueKind {
    Integer,
    OptionalInteger,
    Text,
    OptionalText,
}

struct ColumnSpec {
    name: &'static str,
    kind: ValueKind,
}

struct TableSpec {
    name: &'static str,
    columns: &'static [ColumnSpec],
    order_by: &'static [&'static str],
    serial_id: bool,
}

enum CellValue {
    Integer(i64),
    OptionalInteger(Option<i64>),
    Text(String),
    OptionalText(Option<String>),
}

const USERS_COLUMNS: &[ColumnSpec] = &[
    ColumnSpec {
        name: "id",
        kind: ValueKind::Integer,
    },
    ColumnSpec {
        name: "username",
        kind: ValueKind::Text,
    },
    ColumnSpec {
        name: "password_hash",
        kind: ValueKind::Text,
    },
    ColumnSpec {
        name: "nickname",
        kind: ValueKind::Text,
    },
    ColumnSpec {
        name: "role",
        kind: ValueKind::Text,
    },
    ColumnSpec {
        name: "status",
        kind: ValueKind::Text,
    },
    ColumnSpec {
        name: "must_change_credentials",
        kind: ValueKind::Integer,
    },
    ColumnSpec {
        name: "token_version",
        kind: ValueKind::Integer,
    },
    ColumnSpec {
        name: "created_at",
        kind: ValueKind::Text,
    },
    ColumnSpec {
        name: "updated_at",
        kind: ValueKind::Text,
    },
];

const NODE_GROUPS_COLUMNS: &[ColumnSpec] = &[
    ColumnSpec {
        name: "id",
        kind: ValueKind::Integer,
    },
    ColumnSpec {
        name: "name",
        kind: ValueKind::Text,
    },
    ColumnSpec {
        name: "sort_order",
        kind: ValueKind::Integer,
    },
    ColumnSpec {
        name: "created_at",
        kind: ValueKind::Text,
    },
    ColumnSpec {
        name: "updated_at",
        kind: ValueKind::Text,
    },
];

const TEMPLATES_COLUMNS: &[ColumnSpec] = &[
    ColumnSpec {
        name: "id",
        kind: ValueKind::Integer,
    },
    ColumnSpec {
        name: "name",
        kind: ValueKind::Text,
    },
    ColumnSpec {
        name: "kind",
        kind: ValueKind::Text,
    },
    ColumnSpec {
        name: "content",
        kind: ValueKind::Text,
    },
    ColumnSpec {
        name: "is_builtin",
        kind: ValueKind::Integer,
    },
    ColumnSpec {
        name: "created_at",
        kind: ValueKind::Text,
    },
    ColumnSpec {
        name: "updated_at",
        kind: ValueKind::Text,
    },
];

const SUBSCRIPTION_GROUPS_COLUMNS: &[ColumnSpec] = &[
    ColumnSpec {
        name: "id",
        kind: ValueKind::Integer,
    },
    ColumnSpec {
        name: "name",
        kind: ValueKind::Text,
    },
    ColumnSpec {
        name: "sort_order",
        kind: ValueKind::Integer,
    },
    ColumnSpec {
        name: "created_at",
        kind: ValueKind::Text,
    },
    ColumnSpec {
        name: "updated_at",
        kind: ValueKind::Text,
    },
];

const NODES_COLUMNS: &[ColumnSpec] = &[
    ColumnSpec {
        name: "id",
        kind: ValueKind::Integer,
    },
    ColumnSpec {
        name: "name",
        kind: ValueKind::Text,
    },
    ColumnSpec {
        name: "protocol",
        kind: ValueKind::Text,
    },
    ColumnSpec {
        name: "raw_link",
        kind: ValueKind::Text,
    },
    ColumnSpec {
        name: "server",
        kind: ValueKind::Text,
    },
    ColumnSpec {
        name: "port",
        kind: ValueKind::Integer,
    },
    ColumnSpec {
        name: "enabled",
        kind: ValueKind::Integer,
    },
    ColumnSpec {
        name: "group_id",
        kind: ValueKind::OptionalInteger,
    },
    ColumnSpec {
        name: "source_type",
        kind: ValueKind::Text,
    },
    ColumnSpec {
        name: "source_ref",
        kind: ValueKind::OptionalText,
    },
    ColumnSpec {
        name: "upstream_missing",
        kind: ValueKind::Integer,
    },
    ColumnSpec {
        name: "fingerprint",
        kind: ValueKind::Text,
    },
    ColumnSpec {
        name: "fingerprint_scope",
        kind: ValueKind::Integer,
    },
    ColumnSpec {
        name: "settings_json",
        kind: ValueKind::Text,
    },
    ColumnSpec {
        name: "remark",
        kind: ValueKind::Text,
    },
    ColumnSpec {
        name: "last_latency_ms",
        kind: ValueKind::OptionalInteger,
    },
    ColumnSpec {
        name: "last_latency_status",
        kind: ValueKind::OptionalText,
    },
    ColumnSpec {
        name: "last_latency_message",
        kind: ValueKind::OptionalText,
    },
    ColumnSpec {
        name: "last_latency_tested_at",
        kind: ValueKind::OptionalText,
    },
    ColumnSpec {
        name: "created_at",
        kind: ValueKind::Text,
    },
    ColumnSpec {
        name: "updated_at",
        kind: ValueKind::Text,
    },
];

const SUBSCRIPTIONS_COLUMNS: &[ColumnSpec] = &[
    ColumnSpec {
        name: "id",
        kind: ValueKind::Integer,
    },
    ColumnSpec {
        name: "name",
        kind: ValueKind::Text,
    },
    ColumnSpec {
        name: "token",
        kind: ValueKind::Text,
    },
    ColumnSpec {
        name: "description",
        kind: ValueKind::Text,
    },
    ColumnSpec {
        name: "default_client",
        kind: ValueKind::OptionalText,
    },
    ColumnSpec {
        name: "template_id",
        kind: ValueKind::OptionalInteger,
    },
    ColumnSpec {
        name: "group_id",
        kind: ValueKind::OptionalInteger,
    },
    ColumnSpec {
        name: "enabled",
        kind: ValueKind::Integer,
    },
    ColumnSpec {
        name: "expires_at",
        kind: ValueKind::OptionalText,
    },
    ColumnSpec {
        name: "include_rules",
        kind: ValueKind::Integer,
    },
    ColumnSpec {
        name: "portal_enabled",
        kind: ValueKind::Integer,
    },
    ColumnSpec {
        name: "portal_slug",
        kind: ValueKind::OptionalText,
    },
    ColumnSpec {
        name: "portal_access_code_hash",
        kind: ValueKind::OptionalText,
    },
    ColumnSpec {
        name: "created_at",
        kind: ValueKind::Text,
    },
    ColumnSpec {
        name: "updated_at",
        kind: ValueKind::Text,
    },
];

const UPSTREAM_SUBSCRIPTIONS_COLUMNS: &[ColumnSpec] = &[
    ColumnSpec {
        name: "id",
        kind: ValueKind::Integer,
    },
    ColumnSpec {
        name: "name",
        kind: ValueKind::Text,
    },
    ColumnSpec {
        name: "url",
        kind: ValueKind::Text,
    },
    ColumnSpec {
        name: "group_id",
        kind: ValueKind::OptionalInteger,
    },
    ColumnSpec {
        name: "enabled",
        kind: ValueKind::Integer,
    },
    ColumnSpec {
        name: "sync_enabled",
        kind: ValueKind::Integer,
    },
    ColumnSpec {
        name: "sync_interval_minutes",
        kind: ValueKind::Integer,
    },
    ColumnSpec {
        name: "remark",
        kind: ValueKind::Text,
    },
    ColumnSpec {
        name: "last_imported_at",
        kind: ValueKind::OptionalText,
    },
    ColumnSpec {
        name: "last_import_status",
        kind: ValueKind::OptionalText,
    },
    ColumnSpec {
        name: "last_import_message",
        kind: ValueKind::OptionalText,
    },
    ColumnSpec {
        name: "last_import_imported",
        kind: ValueKind::Integer,
    },
    ColumnSpec {
        name: "last_import_updated",
        kind: ValueKind::Integer,
    },
    ColumnSpec {
        name: "last_import_disabled",
        kind: ValueKind::Integer,
    },
    ColumnSpec {
        name: "last_import_skipped",
        kind: ValueKind::Integer,
    },
    ColumnSpec {
        name: "last_import_failed",
        kind: ValueKind::Integer,
    },
    ColumnSpec {
        name: "template_id",
        kind: ValueKind::OptionalInteger,
    },
    ColumnSpec {
        name: "template_name",
        kind: ValueKind::OptionalText,
    },
    ColumnSpec {
        name: "created_at",
        kind: ValueKind::Text,
    },
    ColumnSpec {
        name: "updated_at",
        kind: ValueKind::Text,
    },
];

const SUBSCRIPTION_NODES_COLUMNS: &[ColumnSpec] = &[
    ColumnSpec {
        name: "subscription_id",
        kind: ValueKind::Integer,
    },
    ColumnSpec {
        name: "node_id",
        kind: ValueKind::Integer,
    },
    ColumnSpec {
        name: "sort_order",
        kind: ValueKind::Integer,
    },
];

const SUBSCRIPTION_NODE_GROUPS_COLUMNS: &[ColumnSpec] = &[
    ColumnSpec {
        name: "subscription_id",
        kind: ValueKind::Integer,
    },
    ColumnSpec {
        name: "node_group_id",
        kind: ValueKind::Integer,
    },
    ColumnSpec {
        name: "sort_order",
        kind: ValueKind::Integer,
    },
];

const ACCESS_LOGS_COLUMNS: &[ColumnSpec] = &[
    ColumnSpec {
        name: "id",
        kind: ValueKind::Integer,
    },
    ColumnSpec {
        name: "subscription_id",
        kind: ValueKind::Integer,
    },
    ColumnSpec {
        name: "client_type",
        kind: ValueKind::OptionalText,
    },
    ColumnSpec {
        name: "ip",
        kind: ValueKind::Text,
    },
    ColumnSpec {
        name: "user_agent",
        kind: ValueKind::Text,
    },
    ColumnSpec {
        name: "status",
        kind: ValueKind::Text,
    },
    ColumnSpec {
        name: "requested_at",
        kind: ValueKind::Text,
    },
];

const APP_SETTINGS_COLUMNS: &[ColumnSpec] = &[
    ColumnSpec {
        name: "key",
        kind: ValueKind::Text,
    },
    ColumnSpec {
        name: "value",
        kind: ValueKind::Text,
    },
    ColumnSpec {
        name: "updated_at",
        kind: ValueKind::Text,
    },
];

const NODE_IP_PROBES_COLUMNS: &[ColumnSpec] = &[
    ColumnSpec {
        name: "node_id",
        kind: ValueKind::Integer,
    },
    ColumnSpec {
        name: "status",
        kind: ValueKind::Text,
    },
    ColumnSpec {
        name: "ip",
        kind: ValueKind::OptionalText,
    },
    ColumnSpec {
        name: "ip_version",
        kind: ValueKind::OptionalInteger,
    },
    ColumnSpec {
        name: "exit_ip_revision",
        kind: ValueKind::Integer,
    },
    ColumnSpec {
        name: "country_code",
        kind: ValueKind::OptionalText,
    },
    ColumnSpec {
        name: "country_name",
        kind: ValueKind::OptionalText,
    },
    ColumnSpec {
        name: "country_source",
        kind: ValueKind::OptionalText,
    },
    ColumnSpec {
        name: "intelligence_status",
        kind: ValueKind::OptionalText,
    },
    ColumnSpec {
        name: "intelligence_message",
        kind: ValueKind::OptionalText,
    },
    ColumnSpec {
        name: "risk_ip",
        kind: ValueKind::OptionalText,
    },
    ColumnSpec {
        name: "risk_status",
        kind: ValueKind::OptionalText,
    },
    ColumnSpec {
        name: "scamalytics_fraud_score",
        kind: ValueKind::OptionalInteger,
    },
    ColumnSpec {
        name: "scamalytics_isp_risk_score",
        kind: ValueKind::OptionalInteger,
    },
    ColumnSpec {
        name: "risk_checked_at",
        kind: ValueKind::OptionalText,
    },
    ColumnSpec {
        name: "risk_expires_at_unix_ms",
        kind: ValueKind::OptionalInteger,
    },
    ColumnSpec {
        name: "risk_message",
        kind: ValueKind::OptionalText,
    },
    ColumnSpec {
        name: "risk_traits_json",
        kind: ValueKind::OptionalText,
    },
    ColumnSpec {
        name: "risk_traits_expires_at_unix_ms",
        kind: ValueKind::OptionalInteger,
    },
    ColumnSpec {
        name: "message",
        kind: ValueKind::OptionalText,
    },
    ColumnSpec {
        name: "probed_at",
        kind: ValueKind::Text,
    },
    ColumnSpec {
        name: "country_updated_at",
        kind: ValueKind::OptionalText,
    },
    ColumnSpec {
        name: "intelligence_updated_at",
        kind: ValueKind::OptionalText,
    },
    ColumnSpec {
        name: "updated_at",
        kind: ValueKind::Text,
    },
];

const TABLES: &[TableSpec] = &[
    TableSpec {
        name: "users",
        columns: USERS_COLUMNS,
        order_by: &["id"],
        serial_id: true,
    },
    TableSpec {
        name: "node_groups",
        columns: NODE_GROUPS_COLUMNS,
        order_by: &["id"],
        serial_id: true,
    },
    TableSpec {
        name: "templates",
        columns: TEMPLATES_COLUMNS,
        order_by: &["id"],
        serial_id: true,
    },
    TableSpec {
        name: "subscription_groups",
        columns: SUBSCRIPTION_GROUPS_COLUMNS,
        order_by: &["id"],
        serial_id: true,
    },
    TableSpec {
        name: "nodes",
        columns: NODES_COLUMNS,
        order_by: &["id"],
        serial_id: true,
    },
    TableSpec {
        name: "subscriptions",
        columns: SUBSCRIPTIONS_COLUMNS,
        order_by: &["id"],
        serial_id: true,
    },
    TableSpec {
        name: "upstream_subscriptions",
        columns: UPSTREAM_SUBSCRIPTIONS_COLUMNS,
        order_by: &["id"],
        serial_id: true,
    },
    TableSpec {
        name: "subscription_nodes",
        columns: SUBSCRIPTION_NODES_COLUMNS,
        order_by: &["subscription_id", "sort_order", "node_id"],
        serial_id: false,
    },
    TableSpec {
        name: "subscription_node_groups",
        columns: SUBSCRIPTION_NODE_GROUPS_COLUMNS,
        order_by: &["subscription_id", "sort_order", "node_group_id"],
        serial_id: false,
    },
    TableSpec {
        name: "access_logs",
        columns: ACCESS_LOGS_COLUMNS,
        order_by: &["id"],
        serial_id: true,
    },
    TableSpec {
        name: "app_settings",
        columns: APP_SETTINGS_COLUMNS,
        order_by: &["key"],
        serial_id: false,
    },
    TableSpec {
        name: "node_ip_probes",
        columns: NODE_IP_PROBES_COLUMNS,
        order_by: &["node_id"],
        serial_id: false,
    },
];

pub async fn run_cli(
    arguments: impl Iterator<Item = String>,
) -> Result<(), DatabaseMigrationError> {
    let arguments = arguments.collect::<Vec<_>>();
    let mode = match arguments.as_slice() {
        [command] if command == "check" => MigrationMode::Check,
        [command] if command == "run" => MigrationMode::Run,
        _ => return Err(DatabaseMigrationError::InvalidArguments),
    };
    let config = MigrationConfig::from_env()?;

    match mode {
        MigrationMode::Check => {
            let inspection = preflight(&config).await?;
            print_counts("Source rows", &inspection.counts);
            println!(
                "Migration preflight passed: {} -> {}; target database is empty.",
                kind_name(config.source_kind),
                kind_name(config.target_kind)
            );
            Ok(())
        }
        MigrationMode::Run => {
            require_confirmation()?;
            let report = migrate(&config).await?;
            print_counts("Migrated rows", &report.counts);
            println!(
                "Database migration completed and verified: {} -> {}.",
                kind_name(report.source_kind),
                kind_name(report.target_kind)
            );
            Ok(())
        }
    }
}

impl MigrationConfig {
    fn from_env() -> Result<Self, DatabaseMigrationError> {
        let source_url = required_environment(SOURCE_URL_ENV)?;
        let target_url = required_environment(TARGET_URL_ENV)?;
        if source_url == target_url {
            return Err(DatabaseMigrationError::SameDatabase);
        }

        let source_kind = checked_kind(SOURCE_URL_ENV, &source_url)?;
        let target_kind = checked_kind(TARGET_URL_ENV, &target_url)?;
        Ok(Self {
            source_url,
            target_url,
            source_kind,
            target_kind,
        })
    }
}

async fn preflight(config: &MigrationConfig) -> Result<DatabaseInspection, DatabaseMigrationError> {
    let source = connect_database_pool_uninitialized(&config.source_url, false).await?;
    let source_inspection = match inspect_database(&source, config.source_kind).await {
        Ok(inspection) => inspection,
        Err(error) => {
            source.close().await;
            return Err(error);
        }
    };

    if !source_inspection.missing_tables.is_empty() {
        source.close().await;
        return Err(DatabaseMigrationError::MissingSourceTables(
            source_inspection.missing_tables.join(", "),
        ));
    }
    let validation = validate_source_columns(&source, config.source_kind).await;
    source.close().await;
    validation?;

    let target_inspection = inspect_target(config).await?;
    reject_non_empty_target(&target_inspection)?;

    Ok(source_inspection)
}

async fn migrate(config: &MigrationConfig) -> Result<MigrationReport, DatabaseMigrationError> {
    preflight(config).await?;

    let source = connect_database_pool_uninitialized(&config.source_url, false).await?;
    let target = new_database_pool(&config.target_url).await?;
    let migration = migrate_pools(&source, &target, config.source_kind, config.target_kind).await;
    source.close().await;
    target.close().await;
    let counts = migration?;

    Ok(MigrationReport {
        source_kind: config.source_kind,
        target_kind: config.target_kind,
        counts,
    })
}

async fn migrate_pools(
    source: &DbPool,
    target: &DbPool,
    source_kind: DbKind,
    target_kind: DbKind,
) -> Result<BTreeMap<&'static str, i64>, DatabaseMigrationError> {
    let mut source_transaction = source.begin().await?;
    let mut target_transaction = target.begin().await?;

    configure_source_transaction(&mut source_transaction, source_kind).await?;

    ensure_initialized_target_has_no_business_data(&mut target_transaction, target_kind).await?;
    clear_initialized_target(&mut target_transaction, target_kind).await?;

    let mut counts = BTreeMap::new();
    for table in TABLES {
        let source_count = table_count_in_transaction(
            &mut source_transaction,
            source_kind,
            table,
            "count source rows",
        )
        .await?;
        copy_table(
            &mut source_transaction,
            &mut target_transaction,
            source_kind,
            target_kind,
            table,
            source_count,
        )
        .await?;

        let target_count = table_count_in_transaction(
            &mut target_transaction,
            target_kind,
            table,
            "verify target rows",
        )
        .await?;
        if target_count != source_count {
            return Err(DatabaseMigrationError::CountMismatch {
                table: table.name,
                source_count,
                target_count,
            });
        }
        counts.insert(table.name, source_count);
    }

    reset_postgres_sequences(&mut target_transaction, target_kind).await?;
    source_transaction.rollback().await?;
    target_transaction.commit().await?;
    Ok(counts)
}

async fn configure_source_transaction(
    transaction: &mut Transaction<'_, Any>,
    kind: DbKind,
) -> Result<(), DatabaseMigrationError> {
    match kind {
        DbKind::Sqlite => {
            sqlx::query::<Any>("PRAGMA query_only = ON")
                .execute(&mut **transaction)
                .await?;
        }
        DbKind::Postgres => {
            sqlx::query::<Any>("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
                .execute(&mut **transaction)
                .await?;
        }
        DbKind::MySql => {
            // MySQL uses a repeatable-read transaction by default. The migration
            // command never issues a write against the source connection.
        }
    }
    Ok(())
}

async fn inspect_target(
    config: &MigrationConfig,
) -> Result<DatabaseInspection, DatabaseMigrationError> {
    if config.target_kind == DbKind::Sqlite
        && let Some(path) = sqlite_file_path(&config.target_url)
        && !path.exists()
    {
        return Ok(DatabaseInspection {
            counts: BTreeMap::new(),
            missing_tables: TABLES.iter().map(|table| table.name).collect(),
        });
    }

    let target = connect_database_pool_uninitialized(&config.target_url, false).await?;
    let inspection = inspect_database(&target, config.target_kind).await;
    target.close().await;
    inspection
}

async fn inspect_database(
    pool: &DbPool,
    kind: DbKind,
) -> Result<DatabaseInspection, DatabaseMigrationError> {
    let mut counts = BTreeMap::new();
    let mut missing_tables = Vec::new();

    for table in TABLES {
        if !table_exists(pool, kind, table.name).await? {
            missing_tables.push(table.name);
            continue;
        }
        let count_sql = format!(
            "SELECT COUNT(*) FROM {}",
            quote_identifier(kind, table.name)
        );
        let count = sqlx::query_scalar::<Any, i64>(&count_sql)
            .fetch_one(pool)
            .await
            .map_err(|source| DatabaseMigrationError::TableOperation {
                operation: "count rows",
                table: table.name,
                source,
            })?;
        counts.insert(table.name, count);
    }

    Ok(DatabaseInspection {
        counts,
        missing_tables,
    })
}

async fn validate_source_columns(
    pool: &DbPool,
    kind: DbKind,
) -> Result<(), DatabaseMigrationError> {
    for table in TABLES {
        let columns = table
            .columns
            .iter()
            .map(|column| source_column_expression(kind, column))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT {columns} FROM {} LIMIT 1",
            quote_identifier(kind, table.name)
        );
        let row = sqlx::query::<Any>(&sql)
            .fetch_optional(pool)
            .await
            .map_err(|source| DatabaseMigrationError::TableOperation {
                operation: "validate source columns",
                table: table.name,
                source,
            })?;
        if let Some(row) = row {
            row_values(&row, table).map_err(|source| DatabaseMigrationError::TableOperation {
                operation: "decode source columns",
                table: table.name,
                source,
            })?;
        }
    }
    Ok(())
}

async fn table_exists(
    pool: &DbPool,
    kind: DbKind,
    table: &'static str,
) -> Result<bool, DatabaseMigrationError> {
    let sql = match kind {
        DbKind::Sqlite => "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?",
        DbKind::MySql => {
            "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = DATABASE() AND table_name = ?"
        }
        DbKind::Postgres => {
            "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = current_schema() AND table_name = $1"
        }
    };
    let count = sqlx::query_scalar::<Any, i64>(sql)
        .bind(table)
        .fetch_one(pool)
        .await?;
    Ok(count > 0)
}

fn reject_non_empty_target(inspection: &DatabaseInspection) -> Result<(), DatabaseMigrationError> {
    let occupied = inspection
        .counts
        .iter()
        .filter(|(_, count)| **count > 0)
        .map(|(table, count)| format!("{table}={count}"))
        .collect::<Vec<_>>();
    if occupied.is_empty() {
        Ok(())
    } else {
        Err(DatabaseMigrationError::TargetNotEmpty(occupied.join(", ")))
    }
}

async fn ensure_initialized_target_has_no_business_data(
    transaction: &mut Transaction<'_, Any>,
    kind: DbKind,
) -> Result<(), DatabaseMigrationError> {
    let mut occupied = Vec::new();
    for table in TABLES.iter().filter(|table| table.name != "app_settings") {
        let count =
            table_count_in_transaction(transaction, kind, table, "verify initialized target")
                .await?;
        if count > 0 {
            occupied.push(format!("{}={count}", table.name));
        }
    }

    let key_identifier = quote_identifier(kind, "key");
    let sql = format!("SELECT {key_identifier}, value FROM app_settings ORDER BY {key_identifier}");
    let settings = sqlx::query::<Any>(&sql)
        .fetch_all(&mut **transaction)
        .await
        .map_err(|source| DatabaseMigrationError::TableOperation {
            operation: "verify initialized target settings",
            table: "app_settings",
            source,
        })?;
    for setting in settings {
        let key: String = setting.try_get(0)?;
        let value: String = setting.try_get(1)?;
        if !INITIAL_APP_SETTINGS
            .iter()
            .any(|(expected_key, expected_value)| key == *expected_key && value == *expected_value)
        {
            occupied.push(format!("app_settings.{key}"));
        }
    }

    if occupied.is_empty() {
        Ok(())
    } else {
        Err(DatabaseMigrationError::TargetNotEmpty(occupied.join(", ")))
    }
}

async fn clear_initialized_target(
    transaction: &mut Transaction<'_, Any>,
    kind: DbKind,
) -> Result<(), DatabaseMigrationError> {
    for table in TABLES.iter().rev() {
        let sql = format!("DELETE FROM {}", quote_identifier(kind, table.name));
        sqlx::query::<Any>(&sql)
            .execute(&mut **transaction)
            .await
            .map_err(|source| DatabaseMigrationError::TableOperation {
                operation: "clear initialized target",
                table: table.name,
                source,
            })?;
    }
    Ok(())
}

async fn copy_table(
    source: &mut Transaction<'_, Any>,
    target: &mut Transaction<'_, Any>,
    source_kind: DbKind,
    target_kind: DbKind,
    table: &'static TableSpec,
    source_count: i64,
) -> Result<(), DatabaseMigrationError> {
    let select_columns = table
        .columns
        .iter()
        .map(|column| source_column_expression(source_kind, column))
        .collect::<Vec<_>>()
        .join(", ");
    let order_by = table
        .order_by
        .iter()
        .map(|column| quote_identifier(source_kind, column))
        .collect::<Vec<_>>()
        .join(", ");
    let insert_sql = insert_sql(target_kind, table);

    let mut offset = 0_i64;
    while offset < source_count {
        let select_sql = format!(
            "SELECT {select_columns} FROM {} ORDER BY {order_by} LIMIT {BATCH_SIZE} OFFSET {offset}",
            quote_identifier(source_kind, table.name)
        );
        let rows = sqlx::query::<Any>(&select_sql)
            .fetch_all(&mut **source)
            .await
            .map_err(|source| DatabaseMigrationError::TableOperation {
                operation: "read source rows",
                table: table.name,
                source,
            })?;
        if rows.is_empty() {
            break;
        }

        for row in rows {
            let values = row_values(&row, table).map_err(|source| {
                DatabaseMigrationError::TableOperation {
                    operation: "decode source rows",
                    table: table.name,
                    source,
                }
            })?;
            insert_row(target, table, &insert_sql, values).await?;
        }
        offset += BATCH_SIZE;
    }
    Ok(())
}

fn row_values(row: &AnyRow, table: &TableSpec) -> Result<Vec<CellValue>, sqlx::Error> {
    let mut values = Vec::with_capacity(table.columns.len());
    for column in table.columns {
        let value = match column.kind {
            ValueKind::Integer => CellValue::Integer(row.try_get(column.name)?),
            ValueKind::OptionalInteger => CellValue::OptionalInteger(row.try_get(column.name)?),
            ValueKind::Text => CellValue::Text(row.try_get(column.name)?),
            ValueKind::OptionalText => CellValue::OptionalText(row.try_get(column.name)?),
        };
        values.push(value);
    }
    Ok(values)
}

async fn insert_row(
    target: &mut Transaction<'_, Any>,
    table: &'static TableSpec,
    insert_sql: &str,
    values: Vec<CellValue>,
) -> Result<(), DatabaseMigrationError> {
    let mut query = sqlx::query::<Any>(insert_sql);
    for value in values {
        query = match value {
            CellValue::Integer(value) => query.bind(value),
            CellValue::OptionalInteger(value) => query.bind(value),
            CellValue::Text(value) => query.bind(value),
            CellValue::OptionalText(value) => query.bind(value),
        };
    }
    query.execute(&mut **target).await.map_err(|source| {
        DatabaseMigrationError::TableOperation {
            operation: "write target rows",
            table: table.name,
            source,
        }
    })?;
    Ok(())
}

async fn table_count_in_transaction(
    transaction: &mut Transaction<'_, Any>,
    kind: DbKind,
    table: &'static TableSpec,
    operation: &'static str,
) -> Result<i64, DatabaseMigrationError> {
    let sql = format!(
        "SELECT COUNT(*) FROM {}",
        quote_identifier(kind, table.name)
    );
    sqlx::query_scalar::<Any, i64>(&sql)
        .fetch_one(&mut **transaction)
        .await
        .map_err(|source| DatabaseMigrationError::TableOperation {
            operation,
            table: table.name,
            source,
        })
}

async fn reset_postgres_sequences(
    transaction: &mut Transaction<'_, Any>,
    kind: DbKind,
) -> Result<(), DatabaseMigrationError> {
    if kind != DbKind::Postgres {
        return Ok(());
    }

    for table in TABLES.iter().filter(|table| table.serial_id) {
        let sql = format!(
            "SELECT setval(pg_get_serial_sequence('{0}', 'id'), COALESCE((SELECT MAX(id) FROM \"{0}\"), 1), EXISTS(SELECT 1 FROM \"{0}\"))",
            table.name
        );
        sqlx::query::<Any>(&sql)
            .execute(&mut **transaction)
            .await
            .map_err(|source| DatabaseMigrationError::TableOperation {
                operation: "reset PostgreSQL sequence",
                table: table.name,
                source,
            })?;
    }
    Ok(())
}

fn insert_sql(kind: DbKind, table: &TableSpec) -> String {
    let columns = table
        .columns
        .iter()
        .map(|column| quote_identifier(kind, column.name))
        .collect::<Vec<_>>()
        .join(", ");
    let placeholders = (1..=table.columns.len())
        .map(|index| match kind {
            DbKind::Postgres => format!("${index}"),
            DbKind::Sqlite | DbKind::MySql => "?".to_string(),
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "INSERT INTO {} ({columns}) VALUES ({placeholders})",
        quote_identifier(kind, table.name)
    )
}

fn source_column_expression(kind: DbKind, column: &ColumnSpec) -> String {
    let identifier = quote_identifier(kind, column.name);
    if kind == DbKind::MySql
        && matches!(column.kind, ValueKind::Integer | ValueKind::OptionalInteger)
    {
        format!("CAST({identifier} AS SIGNED) AS {identifier}")
    } else {
        identifier
    }
}

fn quote_identifier(kind: DbKind, identifier: &str) -> String {
    debug_assert!(
        identifier
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
    );
    match kind {
        DbKind::MySql => format!("`{identifier}`"),
        DbKind::Sqlite | DbKind::Postgres => format!("\"{identifier}\""),
    }
}

fn required_environment(name: &'static str) -> Result<String, DatabaseMigrationError> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or(DatabaseMigrationError::MissingEnvironment(name))
}

fn checked_kind(
    variable: &'static str,
    database_url: &str,
) -> Result<DbKind, DatabaseMigrationError> {
    if database_url.starts_with("sqlite:")
        || database_url.starts_with("mysql:")
        || database_url.starts_with("mariadb:")
        || database_url.starts_with("postgres:")
        || database_url.starts_with("postgresql:")
    {
        Ok(detect_db_kind(database_url))
    } else {
        Err(DatabaseMigrationError::UnsupportedDatabaseUrl { variable })
    }
}

fn require_confirmation() -> Result<(), DatabaseMigrationError> {
    if env::var(CONFIRM_ENV).as_deref() == Ok(REQUIRED_CONFIRMATION) {
        Ok(())
    } else {
        Err(DatabaseMigrationError::ConfirmationRequired)
    }
}

fn kind_name(kind: DbKind) -> &'static str {
    match kind {
        DbKind::Sqlite => "SQLite",
        DbKind::MySql => "MySQL/MariaDB",
        DbKind::Postgres => "PostgreSQL",
    }
}

fn print_counts(title: &str, counts: &BTreeMap<&'static str, i64>) {
    println!("{title}:");
    for table in TABLES {
        println!(
            "  {:<28} {}",
            table.name,
            counts.get(table.name).unwrap_or(&0)
        );
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
    };

    use sqlx::Executor;

    use super::{
        DbKind, MigrationConfig, connect_database_pool_uninitialized, migrate, quote_identifier,
    };

    static TEST_DATABASE_ID: AtomicU64 = AtomicU64::new(1);

    struct SqliteFiles {
        paths: Vec<PathBuf>,
    }

    impl SqliteFiles {
        fn new(paths: Vec<PathBuf>) -> Self {
            Self { paths }
        }
    }

    impl Drop for SqliteFiles {
        fn drop(&mut self) {
            for path in &self.paths {
                remove_sqlite_files(path);
            }
        }
    }

    #[tokio::test]
    async fn migrates_sqlite_to_sqlite_with_all_relationships() {
        let source_path = unique_sqlite_path("source");
        let target_path = unique_sqlite_path("target");
        let _files = SqliteFiles::new(vec![source_path.clone(), target_path.clone()]);
        let source_url = sqlite_url(&source_path);
        let target_url = sqlite_url(&target_path);
        create_sqlite_fixture(&source_url).await;

        let report = migrate(&MigrationConfig {
            source_url,
            target_url: target_url.clone(),
            source_kind: DbKind::Sqlite,
            target_kind: DbKind::Sqlite,
        })
        .await
        .expect("SQLite migration should succeed");

        assert_eq!(report.counts["users"], 1);
        assert_eq!(report.counts["nodes"], 2);
        assert_eq!(report.counts["subscriptions"], 1);
        assert_eq!(report.counts["subscription_nodes"], 1);
        assert_eq!(report.counts["subscription_node_groups"], 1);
        assert_eq!(report.counts["upstream_subscriptions"], 1);
        assert_eq!(report.counts["node_ip_probes"], 1);
        verify_fixture(&target_url, DbKind::Sqlite).await;
    }

    #[tokio::test]
    async fn rolls_back_the_target_when_a_relationship_is_invalid() {
        let source_path = unique_sqlite_path("invalid-source");
        let target_path = unique_sqlite_path("rollback-target");
        let _files = SqliteFiles::new(vec![source_path.clone(), target_path.clone()]);
        let source_url = sqlite_url(&source_path);
        let target_url = sqlite_url(&target_path);
        create_sqlite_fixture(&source_url).await;

        let source = connect_database_pool_uninitialized(&source_url, false)
            .await
            .expect("connect fixture source");
        source
            .execute("PRAGMA foreign_keys = OFF")
            .await
            .expect("disable fixture foreign keys");
        source
            .execute(
                "INSERT INTO subscription_nodes (subscription_id, node_id, sort_order) VALUES (50, 999, 99)",
            )
            .await
            .expect("seed invalid source relationship");
        source.close().await;

        let result = migrate(&MigrationConfig {
            source_url,
            target_url: target_url.clone(),
            source_kind: DbKind::Sqlite,
            target_kind: DbKind::Sqlite,
        })
        .await;
        assert!(result.is_err());

        let target = connect_database_pool_uninitialized(&target_url, false)
            .await
            .expect("connect rolled back target");
        let users = sqlx::query_scalar::<sqlx::Any, i64>("SELECT COUNT(*) FROM users")
            .fetch_one(&target)
            .await
            .expect("count rolled back users");
        let subscriptions =
            sqlx::query_scalar::<sqlx::Any, i64>("SELECT COUNT(*) FROM subscriptions")
                .fetch_one(&target)
                .await
                .expect("count rolled back subscriptions");
        assert_eq!(users, 0);
        assert_eq!(subscriptions, 0);
        target.close().await;
    }

    #[tokio::test]
    async fn migrates_sqlite_to_postgres_when_configured() {
        let Ok(target_url) = std::env::var("SUBLINKX_TEST_POSTGRES_MIGRATION_URL") else {
            return;
        };
        let source_path = unique_sqlite_path("postgres-source");
        let _files = SqliteFiles::new(vec![source_path.clone()]);
        let source_url = sqlite_url(&source_path);
        create_sqlite_fixture(&source_url).await;

        let report = migrate(&MigrationConfig {
            source_url,
            target_url: target_url.clone(),
            source_kind: DbKind::Sqlite,
            target_kind: DbKind::Postgres,
        })
        .await
        .expect("PostgreSQL migration should succeed");

        assert_eq!(report.counts["users"], 1);
        assert_eq!(report.counts["access_logs"], 1);
        verify_fixture(&target_url, DbKind::Postgres).await;

        let pool = connect_database_pool_uninitialized(&target_url, false)
            .await
            .expect("connect migrated PostgreSQL database");
        let inserted_id = sqlx::query_scalar::<sqlx::Any, i64>(
            r#"
            INSERT INTO users (
                username, password_hash, nickname, role, status,
                must_change_credentials, token_version, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id
            "#,
        )
        .bind("after-migration")
        .bind("fixture-hash")
        .bind("After Migration")
        .bind("admin")
        .bind("active")
        .bind(0_i64)
        .bind(0_i64)
        .bind("2026-08-30T00:00:00Z")
        .bind("2026-08-30T00:00:00Z")
        .fetch_one(&pool)
        .await
        .expect("PostgreSQL sequence should accept a new row");
        assert!(inserted_id > 5);
        pool.close().await;
    }

    #[tokio::test]
    async fn migrates_sqlite_to_mysql_and_back_when_configured() {
        let Ok(mysql_url) = std::env::var("SUBLINKX_TEST_MYSQL_MIGRATION_URL") else {
            return;
        };
        let source_path = unique_sqlite_path("mysql-source");
        let roundtrip_path = unique_sqlite_path("mysql-roundtrip");
        let _files = SqliteFiles::new(vec![source_path.clone(), roundtrip_path.clone()]);
        let source_url = sqlite_url(&source_path);
        let roundtrip_url = sqlite_url(&roundtrip_path);
        create_sqlite_fixture(&source_url).await;

        let into_mysql = migrate(&MigrationConfig {
            source_url,
            target_url: mysql_url.clone(),
            source_kind: DbKind::Sqlite,
            target_kind: DbKind::MySql,
        })
        .await
        .expect("MySQL migration should succeed");
        assert_eq!(into_mysql.counts["nodes"], 2);
        verify_fixture(&mysql_url, DbKind::MySql).await;

        let mysql = connect_database_pool_uninitialized(&mysql_url, false)
            .await
            .expect("connect migrated MySQL database");
        sqlx::query::<sqlx::Any>(
            r#"
            INSERT INTO users (
                username, password_hash, nickname, role, status,
                must_change_credentials, token_version, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind("after-migration")
        .bind("fixture-hash")
        .bind("After Migration")
        .bind("admin")
        .bind("active")
        .bind(0_i64)
        .bind(0_i64)
        .bind("2026-08-30T00:00:00Z")
        .bind("2026-08-30T00:00:00Z")
        .execute(&mysql)
        .await
        .expect("MySQL auto increment should accept a new row");
        let max_id = sqlx::query_scalar::<sqlx::Any, i64>("SELECT MAX(id) FROM users")
            .fetch_one(&mysql)
            .await
            .expect("read MySQL max user id");
        assert!(max_id > 5);
        mysql.close().await;

        let back_to_sqlite = migrate(&MigrationConfig {
            source_url: mysql_url,
            target_url: roundtrip_url.clone(),
            source_kind: DbKind::MySql,
            target_kind: DbKind::Sqlite,
        })
        .await
        .expect("MySQL source migration should succeed");
        assert_eq!(back_to_sqlite.counts["users"], 2);
        verify_fixture(&roundtrip_url, DbKind::Sqlite).await;
    }

    async fn create_sqlite_fixture(database_url: &str) {
        let pool = connect_database_pool_uninitialized(database_url, true)
            .await
            .expect("create fixture database");
        pool.execute("PRAGMA foreign_keys = ON")
            .await
            .expect("enable SQLite foreign keys");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("initialize fixture schema");

        let statements = [
            r#"INSERT INTO users (id, username, password_hash, nickname, role, status, must_change_credentials, token_version, created_at, updated_at) VALUES (5, 'migration-admin', 'fixture-hash', 'Migration Admin', 'admin', 'active', 0, 3, '2026-08-30T00:00:00Z', '2026-08-30T00:00:00Z')"#,
            r#"INSERT INTO node_groups (id, name, sort_order, created_at, updated_at) VALUES (10, '迁移节点组', 7, '2026-08-30T00:00:00Z', '2026-08-30T00:00:00Z')"#,
            r#"INSERT INTO templates (id, name, kind, content, is_builtin, created_at, updated_at) VALUES (20, '迁移模板', 'mihomo', 'proxies: []', 0, '2026-08-30T00:00:00Z', '2026-08-30T00:00:00Z')"#,
            r#"INSERT INTO subscription_groups (id, name, sort_order, created_at, updated_at) VALUES (30, '迁移订阅组', 9, '2026-08-30T00:00:00Z', '2026-08-30T00:00:00Z')"#,
            r#"INSERT INTO nodes (id, name, protocol, raw_link, server, port, enabled, group_id, source_type, source_ref, upstream_missing, fingerprint, fingerprint_scope, settings_json, remark, last_latency_ms, last_latency_status, last_latency_message, last_latency_tested_at, created_at, updated_at) VALUES (40, '迁移节点', 'ss', 'ss://fixture', 'example.test', 443, 1, 10, 'upstream', '60', 0, 'fixture-fingerprint', 10, '{"cipher":"aes-128-gcm"}', 'fixture remark', 88, 'success', NULL, '2026-08-30T00:00:00Z', '2026-08-30T00:00:00Z', '2026-08-30T00:00:00Z')"#,
            r#"INSERT INTO nodes (id, name, protocol, raw_link, server, port, enabled, group_id, source_type, source_ref, upstream_missing, fingerprint, fingerprint_scope, settings_json, remark, last_latency_ms, last_latency_status, last_latency_message, last_latency_tested_at, created_at, updated_at) VALUES (41, '可空字段节点', 'ss', 'ss://nullable-fixture', 'nullable.example.test', 8443, 0, NULL, 'manual', NULL, 0, 'nullable-fixture-fingerprint', 0, '{}', '', NULL, NULL, NULL, NULL, '2026-08-30T00:00:00Z', '2026-08-30T00:00:00Z')"#,
            r#"INSERT INTO subscriptions (id, name, token, description, default_client, template_id, group_id, enabled, expires_at, created_at, updated_at) VALUES (50, '迁移订阅', 'fixture-subscription-token', 'fixture description', 'mihomo', 20, 30, 1, NULL, '2026-08-30T00:00:00Z', '2026-08-30T00:00:00Z')"#,
            r#"INSERT INTO upstream_subscriptions (id, name, url, group_id, enabled, sync_enabled, sync_interval_minutes, remark, last_imported_at, last_import_status, last_import_message, last_import_imported, last_import_updated, last_import_disabled, last_import_skipped, last_import_failed, template_id, template_name, created_at, updated_at) VALUES (60, '迁移上游', 'https://example.test/subscription', 10, 1, 1, 120, 'fixture upstream', '2026-08-30T00:00:00Z', 'success', NULL, 1, 2, 3, 4, 0, 20, '迁移模板', '2026-08-30T00:00:00Z', '2026-08-30T00:00:00Z')"#,
            r#"INSERT INTO subscription_nodes (subscription_id, node_id, sort_order) VALUES (50, 40, 1)"#,
            r#"INSERT INTO subscription_node_groups (subscription_id, node_group_id, sort_order) VALUES (50, 10, 2)"#,
            r#"INSERT INTO access_logs (id, subscription_id, client_type, ip, user_agent, status, requested_at) VALUES (70, 50, 'mihomo', '127.0.0.1', 'migration-test', 'success', '2026-08-30T00:00:00Z')"#,
            r#"INSERT INTO app_settings (key, value, updated_at) VALUES ('migration.fixture', 'preserved', '2026-08-30T00:00:00Z')"#,
            r#"INSERT INTO node_ip_probes (node_id, status, ip, ip_version, country_code, country_name, country_source, intelligence_status, intelligence_message, message, probed_at, country_updated_at, intelligence_updated_at, updated_at) VALUES (40, 'success', '203.0.113.8', 4, 'JP', '日本', 'fixture', 'success', NULL, NULL, '2026-08-30T00:00:00Z', '2026-08-30T00:00:00Z', '2026-08-30T00:00:00Z', '2026-08-30T00:00:00Z')"#,
        ];
        for statement in statements {
            pool.execute(statement).await.expect("seed fixture row");
        }
        pool.close().await;
    }

    async fn verify_fixture(database_url: &str, kind: DbKind) {
        let pool = connect_database_pool_uninitialized(database_url, false)
            .await
            .expect("connect migrated database");
        let placeholder = if kind == DbKind::Postgres { "$1" } else { "?" };
        let token_sql = format!("SELECT token FROM subscriptions WHERE id = {placeholder}");
        let token = sqlx::query_scalar::<sqlx::Any, String>(&token_sql)
            .bind(50_i64)
            .fetch_one(&pool)
            .await
            .expect("subscription token should be preserved");
        assert_eq!(token, "fixture-subscription-token");

        let related_nodes = sqlx::query_scalar::<sqlx::Any, i64>(
            "SELECT COUNT(*) FROM subscription_nodes WHERE subscription_id = 50 AND node_id = 40",
        )
        .fetch_one(&pool)
        .await
        .expect("subscription relation should be preserved");
        assert_eq!(related_nodes, 1);

        let setting_sql = format!(
            "SELECT value FROM app_settings WHERE {} = 'migration.fixture'",
            quote_identifier(kind, "key")
        );
        let setting = sqlx::query_scalar::<sqlx::Any, String>(&setting_sql)
            .fetch_one(&pool)
            .await
            .expect("custom setting should be preserved");
        assert_eq!(setting, "preserved");

        let country = sqlx::query_scalar::<sqlx::Any, String>(
            "SELECT country_code FROM node_ip_probes WHERE node_id = 40",
        )
        .fetch_one(&pool)
        .await
        .expect("IP probe should be preserved");
        assert_eq!(country, "JP");
        pool.close().await;
    }

    fn unique_sqlite_path(label: &str) -> PathBuf {
        let id = TEST_DATABASE_ID.fetch_add(1, Ordering::Relaxed);
        PathBuf::from(format!(
            "data/database-migration-{label}-{}-{id}.db",
            std::process::id()
        ))
    }

    fn sqlite_url(path: &Path) -> String {
        format!("sqlite://{}", path.to_string_lossy().replace('\\', "/"))
    }

    fn remove_sqlite_files(path: &Path) {
        for suffix in ["", "-shm", "-wal"] {
            let candidate = PathBuf::from(format!("{}{suffix}", path.display()));
            let _ = fs::remove_file(candidate);
        }
    }
}
