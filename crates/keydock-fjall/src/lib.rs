#![forbid(unsafe_code)]

//! Fjall-backed storage adapter.

pub mod codec;
pub mod error;
pub mod gc;
pub mod layout;
pub mod repos;
pub mod store;

pub use error::FjallError;
pub use store::FjallStore;
