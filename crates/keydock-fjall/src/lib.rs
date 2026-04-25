#![forbid(unsafe_code)]

//! Fjall-backed storage adapter.

pub mod codec;
pub mod error;
pub mod gc;
pub mod layout;
mod locks;
pub mod repos;
pub mod store;

#[cfg(test)]
mod store_test;

pub use error::FjallError;
pub use gc::GcSweeper;
pub use store::FjallStore;
