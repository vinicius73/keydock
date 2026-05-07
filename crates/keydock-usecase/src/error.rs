use thiserror::Error;

#[derive(Debug, Error)]
pub enum UseCaseError {
    #[error("not implemented")]
    NotImplemented,

    #[error("not found")]
    NotFound,

    #[error(transparent)]
    Domain(#[from] keydock_domain::DomainError),

    /// Persistence or adapter failure (message is sanitized; no secrets).
    #[error("storage error: {0}")]
    Storage(String),
}

/// Authentication resolution failure (caller maps to HTTP 401).
#[derive(Debug, Error)]
pub enum AuthError {
    #[error("invalid credential")]
    InvalidCredential,

    /// Root or signing key bytes are unusable for HMAC (e.g. empty key material).
    #[error("invalid key material for credential hashing")]
    InvalidKeyMaterial,
}

/// Temporary token mint or verify failure.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TokenError {
    #[error("invalid token format")]
    InvalidFormat,
    #[error("invalid signature")]
    InvalidSignature,
    #[error("token expired")]
    Expired,
    #[error("bucket mismatch")]
    BucketMismatch,
    #[error("generation mismatch")]
    GenerationMismatch,
    #[error("no signing key configured")]
    NoSigningKey,
    #[error("serialization failed")]
    Serialize,
}
