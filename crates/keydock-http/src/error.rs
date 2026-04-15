use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

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

pub fn not_implemented(message: impl Into<String>) -> Response {
    json_error(StatusCode::NOT_IMPLEMENTED, message)
}

pub fn unauthorized() -> Response {
    json_error(StatusCode::UNAUTHORIZED, "unauthorized")
}

pub fn forbidden() -> Response {
    json_error(StatusCode::FORBIDDEN, "forbidden")
}

pub fn not_found() -> Response {
    json_error(StatusCode::NOT_FOUND, "not_found")
}

pub fn bad_request() -> Response {
    json_error(StatusCode::BAD_REQUEST, "bad_request")
}

pub fn service_unavailable() -> Response {
    json_error(StatusCode::SERVICE_UNAVAILABLE, "service_unavailable")
}

pub fn internal_error() -> Response {
    json_error(StatusCode::INTERNAL_SERVER_ERROR, "internal_error")
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
