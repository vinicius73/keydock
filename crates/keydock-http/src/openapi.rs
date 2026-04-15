use utoipa::OpenApi;

use crate::error::{ErrorBody, ErrorDetail};
use crate::routes::health::{self, HealthResponse};
use crate::routes::keys::{self, PutKeyParams};

#[derive(OpenApi)]
#[openapi(
    paths(
        health::health_check,
        keys::get_key,
        keys::put_key_openapi,
        keys::delete_key,
    ),
    components(schemas(HealthResponse, ErrorBody, ErrorDetail, PutKeyParams))
)]
pub struct ApiDoc;
