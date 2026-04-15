use axum::Json;
use axum::extract::State;
use keydock_state::AppState;
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
}

/// Liveness probe (no dependencies; does not check storage).
#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "OK", body = HealthResponse)
    )
)]
pub async fn health_check(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: state.version(),
    })
}
