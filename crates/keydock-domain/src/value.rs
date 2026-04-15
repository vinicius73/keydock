use bytes::Bytes;
use serde::{Deserialize, Serialize};

use crate::DomainError;

/// Maximum value length for the public HTTP contract.
pub const MAX_VALUE_BYTES: usize = 16 * 1024;

/// Stored value with basic classification for response shaping.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredValue {
    #[serde(with = "serde_bytes")]
    pub payload: Bytes,
    pub kind: ValueKind,
}

mod serde_bytes {
    use bytes::Bytes;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &Bytes, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(bytes.as_ref())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Bytes, D::Error>
    where
        D: Deserializer<'de>,
    {
        let v = <Vec<u8>>::deserialize(deserializer)?;
        Ok(Bytes::from(v))
    }
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
