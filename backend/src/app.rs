use axum::{
    Router,
    http::{Method, header},
    routing::get,
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::{api, state::AppState};

pub fn build_app(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin([
            "http://127.0.0.1:4173".parse().expect("valid CORS origin"),
            "http://localhost:4173".parse().expect("valid CORS origin"),
            "http://127.0.0.1:5173".parse().expect("valid CORS origin"),
            "http://localhost:5173".parse().expect("valid CORS origin"),
        ])
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]);

    Router::new()
        .route("/healthz", get(api::health::healthz))
        .route("/s/{token}", get(api::exports::get_subscription))
        .route("/api/v1/version", get(api::version::version))
        .route(
            "/api/v1/settings",
            get(api::settings::get).put(api::settings::update),
        )
        .route(
            "/api/v1/settings/mihomo-core",
            get(api::settings::mihomo_core_status),
        )
        .route(
            "/api/v1/settings/mihomo-core/download",
            axum::routing::post(api::settings::download_mihomo_core),
        )
        .route("/api/v1/auth/login", axum::routing::post(api::auth::login))
        .route("/api/v1/auth/me", get(api::auth::me))
        .route(
            "/api/v1/auth/change-credentials",
            axum::routing::post(api::auth::change_credentials),
        )
        .route(
            "/api/v1/nodes",
            get(api::nodes::list).post(api::nodes::create),
        )
        .route(
            "/api/v1/nodes/import-subscription",
            axum::routing::post(api::nodes::import_from_subscription),
        )
        .route(
            "/api/v1/nodes/test-latency",
            axum::routing::post(api::nodes::test_latency_batch),
        )
        .route(
            "/api/v1/nodes/{id}/test-latency",
            axum::routing::post(api::nodes::test_latency),
        )
        .route(
            "/api/v1/nodes/{id}",
            get(api::nodes::get)
                .put(api::nodes::update)
                .delete(api::nodes::delete),
        )
        .route(
            "/api/v1/node-groups",
            get(api::groups::list_node_groups).post(api::groups::create_node_group),
        )
        .route(
            "/api/v1/node-groups/{id}",
            axum::routing::put(api::groups::update_node_group)
                .delete(api::groups::delete_node_group),
        )
        .route(
            "/api/v1/subscriptions",
            get(api::subscriptions::list).post(api::subscriptions::create),
        )
        .route(
            "/api/v1/subscriptions/{id}",
            get(api::subscriptions::get)
                .put(api::subscriptions::update)
                .delete(api::subscriptions::delete),
        )
        .route(
            "/api/v1/subscriptions/{id}/rotate-token",
            axum::routing::post(api::subscriptions::rotate_token),
        )
        .route(
            "/api/v1/subscriptions/{id}/renew",
            axum::routing::post(api::subscriptions::renew),
        )
        .route(
            "/api/v1/subscription-groups",
            get(api::groups::list_subscription_groups).post(api::groups::create_subscription_group),
        )
        .route(
            "/api/v1/subscription-groups/{id}",
            axum::routing::put(api::groups::update_subscription_group)
                .delete(api::groups::delete_subscription_group),
        )
        .route(
            "/api/v1/templates",
            get(api::templates::list).post(api::templates::create),
        )
        .route(
            "/api/v1/templates/{id}",
            get(api::templates::get)
                .put(api::templates::update)
                .delete(api::templates::delete),
        )
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
