use axum::body::Body;
use axum::extract::Path;
use axum::extract::State;
use axum::http::Response;
use keydock_state::AppState;

use crate::error::not_implemented;

pub async fn list_bucket(State(_state): State<AppState>, Path(bucket): Path<String>) -> Response<Body> {
    not_implemented(format!("GET /{bucket} (list)"))
}
