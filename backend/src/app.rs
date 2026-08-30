use axum::{
    Router,
    http::{HeaderName, HeaderValue, Method, Request, Uri, header},
    routing::get,
};
use tower_http::{cors::CorsLayer, set_header::SetResponseHeaderLayer, trace::TraceLayer};

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
        .allow_headers([
            header::AUTHORIZATION,
            header::CACHE_CONTROL,
            header::CONTENT_TYPE,
            header::PRAGMA,
            HeaderName::from_static("x-csrf-token"),
        ])
        .expose_headers([HeaderName::from_static("x-csrf-token")])
        .allow_credentials(true);

    Router::new()
        .route("/healthz", get(api::health::healthz))
        .route("/s/{token}", get(api::exports::get_subscription))
        .route("/api/v1/version", get(api::version::version))
        .route("/api/v1/dashboard/stats", get(api::dashboard::stats))
        .route(
            "/api/v1/version/update-check",
            get(api::version::update_check),
        )
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
        .route(
            "/api/v1/auth/logout",
            axum::routing::post(api::auth::logout),
        )
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
            "/api/v1/upstream-subscriptions",
            get(api::upstream_subscriptions::list).post(api::upstream_subscriptions::create),
        )
        .route(
            "/api/v1/upstream-subscriptions/{id}",
            axum::routing::put(api::upstream_subscriptions::update)
                .delete(api::upstream_subscriptions::delete),
        )
        .route(
            "/api/v1/upstream-subscriptions/{id}/import",
            axum::routing::post(api::upstream_subscriptions::import),
        )
        .route(
            "/api/v1/nodes/move",
            axum::routing::post(api::nodes::move_batch),
        )
        .route(
            "/api/v1/nodes/test-latency",
            axum::routing::post(api::nodes::test_latency_batch),
        )
        .route(
            "/api/v1/nodes/test-latency-stream",
            axum::routing::post(api::nodes::test_latency_batch_stream),
        )
        .route(
            "/api/v1/nodes/auto-latency/cancel",
            axum::routing::post(api::nodes::cancel_auto_latency),
        )
        .route(
            "/api/v1/nodes/{id}/test-latency",
            axum::routing::post(api::nodes::test_latency),
        )
        .route(
            "/api/v1/connectivity-tests/presets",
            get(api::connectivity_tests::presets),
        )
        .route(
            "/api/v1/connectivity-tests/stream",
            axum::routing::post(api::connectivity_tests::stream),
        )
        .route(
            "/api/v1/connectivity-tests/cancel",
            axum::routing::post(api::connectivity_tests::cancel),
        )
        .route("/api/v1/node-ip-probes", get(api::node_ip_probes::list))
        .route(
            "/api/v1/node-ip-probes/stream",
            axum::routing::post(api::node_ip_probes::stream),
        )
        .route(
            "/api/v1/node-ip-probes/cancel",
            axum::routing::post(api::node_ip_probes::cancel),
        )
        .route(
            "/api/v1/node-ip-probes/intelligence/status",
            get(api::node_ip_probes::intelligence_status),
        )
        .route(
            "/api/v1/node-ip-probes/intelligence/refresh",
            axum::routing::post(api::node_ip_probes::refresh_intelligence),
        )
        .route(
            "/api/v1/node-ip-probes/{id}/country",
            axum::routing::put(api::node_ip_probes::update_country),
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
            "/api/v1/subscriptions/{id}/export",
            get(api::subscriptions::export),
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
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store, no-cache, must-revalidate, proxy-revalidate"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::PRAGMA,
            HeaderValue::from_static("no-cache"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::EXPIRES,
            HeaderValue::from_static("0"),
        ))
        .layer(cors)
        .layer(
            TraceLayer::new_for_http().make_span_with(|request: &Request<_>| {
                tracing::info_span!(
                    "request",
                    method = %request.method(),
                    path = %redacted_request_path(request.uri()),
                )
            }),
        )
        .with_state(state)
}

fn redacted_request_path(uri: &Uri) -> String {
    let path = uri.path();
    if path == "/s" || path.starts_with("/s/") {
        "/s/<redacted>".to_string()
    } else {
        path.to_string()
    }
}

#[cfg(test)]
mod tests {
    use axum::http::Uri;

    use super::redacted_request_path;

    #[test]
    fn redacts_subscription_token_from_trace_path() {
        let uri = Uri::from_static("/s/secret-token?target=mihomo");

        assert_eq!(redacted_request_path(&uri), "/s/<redacted>");
    }

    #[test]
    fn drops_query_from_non_subscription_trace_path() {
        let uri = Uri::from_static("/api/v1/nodes?token=accidental");

        assert_eq!(redacted_request_path(&uri), "/api/v1/nodes");
    }
}
