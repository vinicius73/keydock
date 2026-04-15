use bytes::Bytes;
use serde::{Deserialize, Serialize};

use crate::DomainError;

/// Maximum key length for the public HTTP contract.
pub const MAX_KEY_BYTES: usize = 128;

/// A key scoped to a bucket (opaque bytes, URL-encoded on the wire).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Key(#[serde(with = "serde_bytes")] Bytes);

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
