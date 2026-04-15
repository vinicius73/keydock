use axum::Router;
use axum::http::{HeaderValue, Method, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use keydock_state::AppState;
use metrics_exporter_prometheus::PrometheusHandle;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;

use crate::openapi::ApiDoc;
use crate::routes::{buckets, health, keys, tokens};

/// Builds the HTTP service with standard middleware and routes.
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
        .route("/health", get(health::health_check))
        .route(
            "/openapi.json",
            get(|| async { JsonOpenApi(ApiDoc::openapi()) }),
        )
        .route(
            "/metrics",
            get(move || {
                let prom = prom.clone();
                async move {
                    let metrics_data = prom.render();
                    (
                        [(
                            axum::http::header::CONTENT_TYPE,
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
                .put(keys::put_key)
                .delete(keys::delete_key)
                .patch(keys::patch_key),
        )
        .route(
            "/{bucket}",
            get(buckets::list_bucket)
                .patch(buckets::update_policy)
                .delete(buckets::delete_bucket),
        )
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}

/// JSON OpenAPI document as Axum response.
struct JsonOpenApi(utoipa::openapi::OpenApi);

impl IntoResponse for JsonOpenApi {
    fn into_response(self) -> axum::response::Response {
        match serde_json::to_string(&self.0) {
            Ok(s) => (
                [(
                    axum::http::header::CONTENT_TYPE,
                    HeaderValue::from_static("application/json"),
                )],
                s,
            )
                .into_response(),
            Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        }
    }
}
