use axum::{
    Router,
    http::{HeaderValue, Method, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use keydock_state::AppState;
use metrics_exporter_prometheus::PrometheusHandle;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::instrument;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::error::not_implemented;
use crate::openapi::ApiDoc;
use crate::routes::{buckets, health, keys, tokens};

/// Placeholder until M5 exposes `GET /{bucket}` for bucket policy.
#[instrument(skip_all, name = "router::get_bucket_reserved_for_m5")]
async fn get_bucket_reserved_for_m5() -> Response {
    not_implemented("GET /{bucket} reserved for M5")
}

/// Builds the HTTP service with standard middleware and routes.
#[instrument(skip_all)]
pub fn build_router(state: AppState, prometheus: PrometheusHandle) -> Router {
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

    Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .route("/health", get(health::health_check))
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
        .route("/", post(buckets::create_bucket))
        .route("/{bucket}/tokens/", post(tokens::create_token))
        .route(
            "/{bucket}/{key}",
            get(keys::get_key)
                .post(keys::put_key)
                .put(keys::put_key)
                .delete(keys::delete_key)
                .patch(keys::patch_key),
        )
        .route("/{bucket}/", get(buckets::list_bucket))
        .route(
            "/{bucket}",
            get(get_bucket_reserved_for_m5)
                .patch(buckets::update_policy)
                .delete(buckets::delete_bucket),
        )
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}
