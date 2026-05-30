use serde::{Deserialize, Serialize};

use crate::domain::group::GroupView;

#[derive(Debug, Deserialize)]
pub struct CreateGroupRequest {
    pub name: String,
    pub sort_order: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateGroupRequest {
    pub name: String,
    pub sort_order: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct GroupListResponse {
    pub code: &'static str,
    pub data: Vec<GroupView>,
}

#[derive(Debug, Serialize)]
pub struct GroupResponse {
    pub code: &'static str,
    pub data: GroupView,
}
