use axum::body::Body;
use axum::extract::Path;
use axum::extract::State;
use axum::http::Response;
use keydock_state::AppState;

use crate::error::not_implemented;

pub async fn get_key(
    State(_state): State<AppState>,
    Path((bucket, key)): Path<(String, String)>,
) -> Response<Body> {
    not_implemented(format!("GET /{bucket}/{key}"))
}

pub async fn put_key(
    State(_state): State<AppState>,
    Path((bucket, key)): Path<(String, String)>,
) -> Response<Body> {
    not_implemented(format!("PUT /{bucket}/{key}"))
}

pub async fn delete_key(
    State(_state): State<AppState>,
    Path((bucket, key)): Path<(String, String)>,
) -> Response<Body> {
    not_implemented(format!("DELETE /{bucket}/{key}"))
}
