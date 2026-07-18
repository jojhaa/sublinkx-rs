use serde::Serialize;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TemplateRecord {
    pub id: i64,
    pub name: String,
    pub kind: String,
    pub content: String,
    pub is_builtin: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TemplateView {
    pub id: i64,
    pub name: String,
    pub kind: String,
    pub content: String,
    pub is_builtin: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<TemplateRecord> for TemplateView {
    fn from(value: TemplateRecord) -> Self {
        Self {
            id: value.id,
            name: value.name,
            kind: value.kind,
            content: value.content,
            is_builtin: value.is_builtin != 0,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}
