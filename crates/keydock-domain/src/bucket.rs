use serde::{Deserialize, Serialize};

use crate::DomainError;

/// Stable bucket identifier (opaque to HTTP; validated in domain).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BucketId(pub String);

impl BucketId {
    /// Creates a bucket id after basic validation.
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let s = raw.into();
        if s.is_empty() {
            return Err(DomainError::InvalidBucketId("empty".into()));
        }
        Ok(Self(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
