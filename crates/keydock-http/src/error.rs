use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub error: String,
}

pub fn not_implemented(message: impl Into<String>) -> Response {
    let body = ErrorBody {
        error: message.into(),
    };
    (StatusCode::NOT_IMPLEMENTED, axum::Json(body)).into_response()
}
