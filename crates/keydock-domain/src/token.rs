use serde::{Deserialize, Serialize};

use crate::{BucketId, Permission};

/// Claims carried by a signed temporary bucket token (non-JWT).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporaryTokenClaims {
    pub version: u8,
    pub bucket: BucketId,
    pub bucket_generation: u64,
    pub allowed_prefix: Vec<u8>,
    pub permissions: Permission,
    #[serde(with = "time::serde::rfc3339")]
    pub iat: time::OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub exp: time::OffsetDateTime,
}

/// Raw signing material (never log, never serialize accidentally).
pub type SigningKey = secrecy::SecretBox<Vec<u8>>;
