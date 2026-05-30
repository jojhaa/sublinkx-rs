use serde::Serialize;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TemplateRecord {
    pub id: i64,
    pub name: String,
    pub kind: String,
    pub content: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TemplateView {
    pub id: i64,
    pub name: String,
    pub kind: String,
    pub content: String,
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
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}
