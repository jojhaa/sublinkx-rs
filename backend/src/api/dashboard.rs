use axum::{Json, extract::State, http::HeaderMap};

use crate::{
    dto::dashboard::{DashboardStats, DashboardStatsResponse},
    errors::AppError,
    services::auth_service,
    state::AppState,
};

pub async fn stats(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<DashboardStatsResponse>, AppError> {
    auth_service::require_user(&state, &headers).await?;
    let (nodes, subscriptions, templates, upstream_subscriptions) =
        sqlx::query_as::<_, (i64, i64, i64, i64)>(
            r#"
            SELECT
                (SELECT COUNT(*) FROM nodes),
                (SELECT COUNT(*) FROM subscriptions),
                (SELECT COUNT(*) FROM templates),
                (SELECT COUNT(*) FROM upstream_subscriptions)
            "#,
        )
        .fetch_one(&state.db)
        .await?;

    Ok(Json(DashboardStatsResponse {
        code: "00000",
        data: DashboardStats {
            nodes,
            subscriptions,
            templates,
            upstream_subscriptions,
        },
    }))
}
