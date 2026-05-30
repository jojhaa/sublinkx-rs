use serde::Serialize;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct GroupRecord {
    pub id: i64,
    pub name: String,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GroupView {
    pub id: i64,
    pub name: String,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

impl From<GroupRecord> for GroupView {
    fn from(value: GroupRecord) -> Self {
        Self {
            id: value.id,
            name: value.name,
            sort_order: value.sort_order,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}
