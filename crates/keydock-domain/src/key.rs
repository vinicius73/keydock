use bytes::Bytes;
use serde::{Deserialize, Serialize};

use crate::DomainError;

/// Maximum key length for the public HTTP contract.
pub const MAX_KEY_BYTES: usize = 128;

/// A key scoped to a bucket (opaque bytes, URL-encoded on the wire).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Key(
    #[serde(
        serialize_with = "crate::serde_bytes::serialize",
        deserialize_with = "crate::serde_bytes::deserialize_key"
    )]
    Bytes,
);

impl Key {
    pub fn from_bytes(data: Bytes) -> Result<Self, DomainError> {
        if data.len() > MAX_KEY_BYTES {
            return Err(DomainError::KeyTooLong {
                max: MAX_KEY_BYTES,
                got: data.len(),
            });
        }
        Ok(Self(data))
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}
