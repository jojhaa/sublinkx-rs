use sqlx::SqlitePool;

use crate::domain::group::GroupRecord;

#[derive(Debug, Clone, Copy)]
pub enum GroupTable {
    Node,
    Subscription,
}

impl GroupTable {
    fn table_name(self) -> &'static str {
        match self {
            Self::Node => "node_groups",
            Self::Subscription => "subscription_groups",
        }
    }

    fn owner_table_name(self) -> &'static str {
        match self {
            Self::Node => "nodes",
            Self::Subscription => "subscriptions",
        }
    }
}

pub struct NewGroupRecord<'a> {
    pub name: &'a str,
    pub sort_order: i64,
    pub created_at: &'a str,
    pub updated_at: &'a str,
}

pub struct UpdateGroupRecord<'a> {
    pub name: &'a str,
    pub sort_order: i64,
    pub updated_at: &'a str,
}

pub async fn list(pool: &SqlitePool, table: GroupTable) -> Result<Vec<GroupRecord>, sqlx::Error> {
    sqlx::query_as::<_, GroupRecord>(&format!(
        "SELECT id, name, sort_order, created_at, updated_at FROM {} ORDER BY sort_order ASC, id ASC",
        table.table_name()
    ))
    .fetch_all(pool)
    .await
}

pub async fn find_by_id(
    pool: &SqlitePool,
    table: GroupTable,
    id: i64,
) -> Result<Option<GroupRecord>, sqlx::Error> {
    sqlx::query_as::<_, GroupRecord>(&format!(
        "SELECT id, name, sort_order, created_at, updated_at FROM {} WHERE id = ?1",
        table.table_name()
    ))
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn find_by_name(
    pool: &SqlitePool,
    table: GroupTable,
    name: &str,
) -> Result<Option<GroupRecord>, sqlx::Error> {
    sqlx::query_as::<_, GroupRecord>(&format!(
        "SELECT id, name, sort_order, created_at, updated_at FROM {} WHERE name = ?1",
        table.table_name()
    ))
    .bind(name)
    .fetch_optional(pool)
    .await
}

pub async fn insert(
    pool: &SqlitePool,
    table: GroupTable,
    item: &NewGroupRecord<'_>,
) -> Result<GroupRecord, sqlx::Error> {
    let result = sqlx::query(&format!(
        "INSERT INTO {} (name, sort_order, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
        table.table_name()
    ))
    .bind(item.name)
    .bind(item.sort_order)
    .bind(item.created_at)
    .bind(item.updated_at)
    .execute(pool)
    .await?;

    find_by_id(pool, table, result.last_insert_rowid())
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

pub async fn update(
    pool: &SqlitePool,
    table: GroupTable,
    id: i64,
    item: &UpdateGroupRecord<'_>,
) -> Result<GroupRecord, sqlx::Error> {
    sqlx::query(&format!(
        "UPDATE {} SET name = ?1, sort_order = ?2, updated_at = ?3 WHERE id = ?4",
        table.table_name()
    ))
    .bind(item.name)
    .bind(item.sort_order)
    .bind(item.updated_at)
    .bind(id)
    .execute(pool)
    .await?;

    find_by_id(pool, table, id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

pub async fn delete(pool: &SqlitePool, table: GroupTable, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query(&format!("DELETE FROM {} WHERE id = ?1", table.table_name()))
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn count_usage(
    pool: &SqlitePool,
    table: GroupTable,
    id: i64,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(&format!(
        "SELECT COUNT(1) FROM {} WHERE group_id = ?1",
        table.owner_table_name()
    ))
    .bind(id)
    .fetch_one(pool)
    .await
}
