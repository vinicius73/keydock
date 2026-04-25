use axum::{
    Router,
    http::{HeaderValue, Method, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use keydock_config::RateLimitConfig;
use keydock_state::AppState;
use metrics_exporter_prometheus::PrometheusHandle;
use real::RealIpLayer;
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::instrument;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use axum::middleware;

use axum_governor::GovernorLayer;

use crate::error::method_not_allowed;
use crate::middleware::metrics;
use crate::openapi::ApiDoc;
use crate::routes::{buckets, health, keys, tokens, txn};

/// Fallback for routes whose path matches but the HTTP method is not allowed.
///
/// Axum's default fallback responds with an empty body, which breaks the
/// `{"error": {...}}` envelope clients rely on. Wiring this via
/// `method_not_allowed_fallback` per-route keeps `OPTIONS` preflight handled
/// by the CORS layer while emitting the shared envelope for unsupported
/// methods on mounted routes.
#[instrument(skip_all, name = "router::method_not_allowed_fallback")]
async fn method_not_allowed_fallback() -> Response {
    method_not_allowed()
}

/// Builds the HTTP service with standard middleware and routes.
#[instrument(skip_all)]
pub fn build_router(
    state: AppState,
    prometheus: PrometheusHandle,
    rate_limit_cfg: RateLimitConfig,
) -> Router {
    let prom = prometheus.clone();
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers(Any);

    let ops_routes = Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .route("/health", get(health::health_check))
        .route("/ready", get(health::readiness_check))
        .route(
            "/metrics",
            get(move || {
                let prom = prom.clone();
                async move {
                    let metrics_data = prom.render();
                    (
                        [(
                            header::CONTENT_TYPE,
                            HeaderValue::from_static("text/plain; version=0.0.4; charset=utf-8"),
                        )],
                        metrics_data,
                    )
                        .into_response()
                }
            }),
        );

    let mut api_routes = Router::new()
        .route("/", post(buckets::create_bucket))
        .route("/{bucket}/tokens/", post(tokens::create_token))
        .route(
            "/{bucket}/{key}",
            get(keys::get_key)
                .head(keys::head_key)
                .post(keys::put_key)
                .put(keys::put_key)
                .delete(keys::delete_key)
                .patch(keys::patch_key),
        )
        .route(
            "/{bucket}/",
            get(buckets::list_bucket).delete(buckets::delete_bucket),
        )
        .route(
            "/{bucket}",
            get(buckets::get_bucket_policy)
                .head(buckets::head_bucket)
                .post(txn::execute_txn)
                .patch(buckets::update_policy)
                .delete(buckets::delete_bucket),
        )
        // Scoped 405 fallback: applies only to the mounted API routes so that
        // `OPTIONS` on other paths still flows through the CORS layer.
        .method_not_allowed_fallback(method_not_allowed_fallback);

    let ops_routes = ops_routes.method_not_allowed_fallback(method_not_allowed_fallback);

    if rate_limit_cfg.enabled {
        api_routes = api_routes.layer(
            ServiceBuilder::new()
                .layer(RealIpLayer::default())
                .layer(GovernorLayer::default()),
        );
    }

    Router::new()
        .merge(ops_routes)
        .merge(api_routes)
        .layer(middleware::from_fn(metrics::track_http_metrics))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}
