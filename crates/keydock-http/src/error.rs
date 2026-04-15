use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use keydock_usecase::UseCaseError;
use serde::Serialize;
use tracing::instrument;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorDetail {
    pub code: u16,
    pub message: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorBody {
    pub error: ErrorDetail,
}

fn json_error(status: StatusCode, message: impl Into<String>) -> Response {
    let msg = message.into();
    let body = ErrorBody {
        error: ErrorDetail {
            code: status.as_u16(),
            message: msg,
        },
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
pub fn not_acceptable() -> Response {
    json_error(StatusCode::NOT_ACCEPTABLE, "not_acceptable")
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
        UseCaseError::NotFound => {
            tracing::debug!("resource not found");
            not_found()
        }
        UseCaseError::Domain(e) => {
            tracing::debug!(error = %e, "domain validation failed");
            bad_request()
        }
        UseCaseError::NotImplemented => {
            tracing::debug!("operation not implemented");
            not_implemented("not implemented")
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
    #[tokio::test]
    async fn error_helpers_status_and_body_shape(#[case] case: JsonErrorCase) {
        use serde_json::json;

        let (resp, expected) = case.response();
        assert_eq!(resp.status(), expected);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("read body");
        let v: serde_json::Value = serde_json::from_slice(&body).expect("json body");
        let code = expected.as_u16();
        let msg = match case {
            JsonErrorCase::Unauthorized => "unauthorized",
            JsonErrorCase::Forbidden => "forbidden",
            JsonErrorCase::NotFound => "not_found",
        };
        assert_eq!(
            v,
            json!({
                "error": {
                    "code": code,
                    "message": msg
                }
            })
        );
    }
}
