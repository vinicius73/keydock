#![forbid(unsafe_code)]

//! Application layer: use cases and ports (no Axum, no Fjall).

pub mod auth;
pub mod buckets;
pub mod context;
mod ct;
pub mod error;
pub mod keys;
pub mod ports;
pub mod tokens;
pub mod txn;

pub use auth::{ResolvedIdentity, hash_credential, resolve, verify_credential};
pub use context::RequestContext;
pub use error::{AuthError, TokenError, UseCaseError};
pub use keys::{KeyService, ListOptsInput, StoredEntry};
pub use ports::{BucketRepository, KeyRepository, ListEntry, ListOpts};
pub use tokens::mint;
