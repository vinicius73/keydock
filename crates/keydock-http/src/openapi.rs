use utoipa::OpenApi;

use crate::routes::health::{self, HealthResponse};

#[derive(OpenApi)]
#[openapi(paths(health::health_check), components(schemas(HealthResponse)))]
pub struct ApiDoc;
