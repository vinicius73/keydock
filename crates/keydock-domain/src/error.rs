use thiserror::Error;

/// Domain-level errors (no transport or IO semantics).
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DomainError {
    #[error("bucket id is invalid: {0}")]
    InvalidBucketId(String),

    #[error("key exceeds maximum length of {max} bytes (got {got})")]
    KeyTooLong { max: usize, got: usize },

    #[error("value exceeds maximum length of {max} bytes (got {got})")]
    ValueTooLong { max: usize, got: usize },

    #[error("TTL is invalid: {0}")]
    InvalidTtl(String),

    #[error("counter operation invalid: {0}")]
    InvalidCounterOp(String),
}
