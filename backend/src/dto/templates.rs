use serde::{Deserialize, Serialize};

use crate::domain::template::TemplateView;

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
}

#[derive(Debug, Serialize)]
pub struct TemplateResponse {
    pub code: &'static str,
    pub data: TemplateView,
}
