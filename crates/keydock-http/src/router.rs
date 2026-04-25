use axum::{
    Router,
    http::{HeaderValue, Method, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use keydock_state::AppState;
use metrics_exporter_prometheus::PrometheusHandle;
use real::RealIpLayer;
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::instrument;
use utoipa_swagger_ui::SwaggerUi;

use axum::middleware;

use axum_governor::GovernorLayer;

use crate::error::method_not_allowed;
use crate::middleware::metrics;
use crate::openapi::{API_PREFIX, openapi};
use crate::rate_limit::RateLimitSettings;
use crate::routes::{buckets, health, keys, tokens, txn};

#[derive(Debug, Clone)]
pub struct RouterOptions {
    pub expose_metrics: bool,
    pub rate_limit: RateLimitSettings,
}

impl Default for RouterOptions {
    fn default() -> Self {
        Self {
            expose_metrics: true,
            rate_limit: RateLimitSettings::default(),
        }
    }
}

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

pub fn build_metrics_router<S>(prometheus: PrometheusHandle) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    let prom = prometheus.clone();
    Router::<S>::new()
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
        )
        .method_not_allowed_fallback(method_not_allowed_fallback)
}

/// Builds the HTTP service with standard middleware and routes.
///
/// When `opts.rate_limit.enabled` is `true`, the caller must initialize the
/// process-global limiter via [`crate::rate_limit::init_rate_limiter`] before
/// serving requests.
#[instrument(skip_all)]
pub fn build_router(state: AppState, prometheus: PrometheusHandle, opts: RouterOptions) -> Router {
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

    let mut ops_routes = Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", openapi()))
        .route("/health", get(health::health_check))
        .route("/ready", get(health::readiness_check));

    if opts.expose_metrics {
        ops_routes = ops_routes.merge(build_metrics_router(prometheus.clone()));
    }

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

    if opts.rate_limit.enabled {
        api_routes = api_routes.layer(
            ServiceBuilder::new()
                .layer(RealIpLayer::default())
                .layer(GovernorLayer::default()),
        );
    }

    Router::new()
        .merge(ops_routes)
        .nest(API_PREFIX, api_routes)
        .layer(middleware::from_fn(metrics::track_http_metrics))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}
