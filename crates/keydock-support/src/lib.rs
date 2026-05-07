#![forbid(unsafe_code)]

//! Small, stable technical helpers shared across crates (no domain semantics).

pub mod clock;

pub use clock::Clock;
