use serde::{Deserialize, Serialize};

use crate::{
    domain::template::TemplateView,
    dto::common::{PaginationMeta, PaginationQuery},
};

#[derive(Debug, Default, Deserialize)]
pub struct TemplateListQuery {
    pub page: Option<u32>,
    pub page_size: Option<u32>,
    pub kind: Option<String>,
}

impl TemplateListQuery {
    pub fn pagination(&self) -> PaginationQuery {
        PaginationQuery::from_options(self.page, self.page_size)
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateTemplateRequest {
    pub name: String,
    pub kind: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTemplateRequest {
    pub name: String,
    pub kind: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct TemplateListResponse {
    pub code: &'static str,
    pub data: Vec<TemplateView>,
    pub pagination: PaginationMeta,
    pub kind_counts: Vec<TemplateKindCount>,
}

#[derive(Debug, Serialize)]
pub struct TemplateKindCount {
    pub kind: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct TemplateResponse {
    pub code: &'static str,
    pub data: TemplateView,
}
