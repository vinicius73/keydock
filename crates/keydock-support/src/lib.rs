#![forbid(unsafe_code)]

//! Small, stable technical helpers shared across crates (no domain semantics).

pub mod clock;
pub mod error;
pub mod secret;

pub use clock::Clock;
pub use error::SupportError;
pub use secret::RedactedSecret;
