use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::DomainError;

/// Time-to-live as an absolute expiration instant (product-layer TTL).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ttl {
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
}

impl Ttl {
    pub fn new(expires_at: OffsetDateTime) -> Result<Self, DomainError> {
        Ok(Self { expires_at })
    }
}
