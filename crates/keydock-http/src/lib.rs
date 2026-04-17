#![forbid(unsafe_code)]
// `Result<T, Response>` is the standard Axum pattern for auth helpers; `Response` is large.
#![allow(clippy::result_large_err)]

//! HTTP edge: routing, DTOs, OpenAPI (no business rules).

pub mod auth;
pub mod error;
pub mod extract;
pub mod metrics;
pub(crate) mod middleware;
pub mod openapi;
pub mod router;
pub mod routes;

pub use metrics::describe_all;
pub use router::build_router;
pub use routes::health::HealthResponse;
