//! JSON encoding for [`BucketPolicy`](keydock_domain::BucketPolicy) in Fjall.

use keydock_domain::{BucketPolicy, Permission, SigningKey};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct StoredPermission {
    read: bool,
    write: bool,
    enumerate: bool,
    delete: bool,
}

impl From<Permission> for StoredPermission {
    fn from(p: Permission) -> Self {
        Self {
            read: p.read,
            write: p.write,
            enumerate: p.enumerate,
            delete: p.delete,
        }
    }
}

impl From<StoredPermission> for Permission {
    fn from(p: StoredPermission) -> Self {
        Self {
            read: p.read,
            write: p.write,
            enumerate: p.enumerate,
            delete: p.delete,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredBucketPolicy {
    secret_key_hash: Option<Vec<u8>>,
    read_key_hash: Option<Vec<u8>>,
    write_key_hash: Option<Vec<u8>>,
    signing_key: Option<Vec<u8>>,
    signing_key_generation: u64,
    default_ttl_secs: Option<u64>,
    anonymous_access: StoredPermission,
}

impl From<&BucketPolicy> for StoredBucketPolicy {
    fn from(p: &BucketPolicy) -> Self {
        Self {
            secret_key_hash: p.secret_key_hash.clone(),
            read_key_hash: p.read_key_hash.clone(),
            write_key_hash: p.write_key_hash.clone(),
            signing_key: p.signing_key.as_ref().map(|k| k.expose_secret().clone()),
            signing_key_generation: p.signing_key_generation,
            default_ttl_secs: p.default_ttl_secs,
            anonymous_access: StoredPermission::from(p.anonymous_access),
        }
    }
}

impl From<StoredBucketPolicy> for BucketPolicy {
    fn from(s: StoredBucketPolicy) -> Self {
        Self {
            default_ttl_secs: s.default_ttl_secs,
            anonymous_access: s.anonymous_access.into(),
            secret_key_hash: s.secret_key_hash,
            read_key_hash: s.read_key_hash,
            write_key_hash: s.write_key_hash,
            signing_key: s.signing_key.map(|b| SigningKey::new(Box::new(b))),
            signing_key_generation: s.signing_key_generation,
        }
    }
}

/// Encode policy to JSON bytes for storage.
pub fn encode_policy(policy: &BucketPolicy) -> Result<Vec<u8>, serde_json::Error> {
    let stored = StoredBucketPolicy::from(policy);
    serde_json::to_vec(&stored)
}

/// Decode policy from stored JSON bytes.
pub fn decode_policy(bytes: &[u8]) -> Result<BucketPolicy, CodecError> {
    let stored: StoredBucketPolicy = serde_json::from_slice(bytes)?;
    Ok(BucketPolicy::from(stored))
}

#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
}
