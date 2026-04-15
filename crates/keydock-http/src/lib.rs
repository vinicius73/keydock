#![forbid(unsafe_code)]

//! HTTP edge: routing, DTOs, OpenAPI (no business rules).

pub mod auth;
pub mod error;
pub mod extract;
pub mod openapi;
pub mod response;
pub mod router;
pub mod routes;

pub use router::build_router;
pub use routes::health::HealthResponse;
