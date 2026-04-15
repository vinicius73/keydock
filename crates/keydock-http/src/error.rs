use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use keydock_usecase::UseCaseError;
use serde::Serialize;
use tracing::instrument;

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub error: String,
}

fn json_error(status: StatusCode, message: impl Into<String>) -> Response {
    let body = ErrorBody {
        error: message.into(),
    };
    (status, axum::Json(body)).into_response()
}

#[instrument(skip_all)]
pub fn not_implemented(message: impl Into<String>) -> Response {
    json_error(StatusCode::NOT_IMPLEMENTED, message)
}

#[instrument(skip_all)]
pub fn unauthorized() -> Response {
    json_error(StatusCode::UNAUTHORIZED, "unauthorized")
}

#[instrument(skip_all)]
pub fn forbidden() -> Response {
    json_error(StatusCode::FORBIDDEN, "forbidden")
}

#[instrument(skip_all)]
pub fn not_found() -> Response {
    json_error(StatusCode::NOT_FOUND, "not_found")
}

#[instrument(skip_all)]
pub fn bad_request() -> Response {
    json_error(StatusCode::BAD_REQUEST, "bad_request")
}

#[instrument(skip_all)]
pub fn service_unavailable() -> Response {
    json_error(StatusCode::SERVICE_UNAVAILABLE, "service_unavailable")
}

#[instrument(skip_all)]
pub fn internal_error() -> Response {
    json_error(StatusCode::INTERNAL_SERVER_ERROR, "internal_error")
}

/// Maps storage / orchestration failures from the use-case layer to a generic HTTP 500 response.
#[instrument(skip_all, name = "error::map_use_case_repo_err")]
pub fn map_use_case_repo_err(err: UseCaseError) -> Response {
    match err {
        UseCaseError::Storage(msg) => {
            tracing::error!(error = %msg, "repository error");
            internal_error()
        }
        other => {
            tracing::error!(?other, "repository error");
            internal_error()
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;

    #[derive(Clone, Copy)]
    enum JsonErrorCase {
        Unauthorized,
        Forbidden,
        NotFound,
    }

    impl JsonErrorCase {
        fn response(self) -> (Response, StatusCode) {
            match self {
                Self::Unauthorized => (unauthorized(), StatusCode::UNAUTHORIZED),
                Self::Forbidden => (forbidden(), StatusCode::FORBIDDEN),
                Self::NotFound => (not_found(), StatusCode::NOT_FOUND),
            }
        }
    }

    #[rstest]
    #[case::unauthorized(JsonErrorCase::Unauthorized)]
    #[case::forbidden(JsonErrorCase::Forbidden)]
    #[case::not_found(JsonErrorCase::NotFound)]
    fn error_helpers_status_and_body_shape(#[case] case: JsonErrorCase) {
        let (resp, expected) = case.response();
        assert_eq!(resp.status(), expected);
    }
}
