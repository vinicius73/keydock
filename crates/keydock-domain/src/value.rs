use bytes::Bytes;
use serde::{Deserialize, Serialize};

use crate::DomainError;

/// Maximum value length for the public HTTP contract.
pub const MAX_VALUE_BYTES: usize = 16 * 1024;

/// Stored value with basic classification for response shaping.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredValue {
    #[serde(
        serialize_with = "crate::serde_bytes::serialize",
        deserialize_with = "crate::serde_bytes::deserialize_value"
    )]
    pub payload: Bytes,
    pub kind: ValueKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueKind {
    Raw,
    Utf8,
    Int64,
    Float64,
    Json,
}

impl StoredValue {
    pub fn new(payload: Bytes, kind: ValueKind) -> Result<Self, DomainError> {
        if payload.len() > MAX_VALUE_BYTES {
            return Err(DomainError::ValueTooLong {
                max: MAX_VALUE_BYTES,
                got: payload.len(),
            });
        }
        Ok(Self { payload, kind })
    }
}
