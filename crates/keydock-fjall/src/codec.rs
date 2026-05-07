//! JSON encoding for [`BucketPolicy`](keydock_domain::BucketPolicy) in Fjall.

use keydock_domain::{BucketPolicy, Permission, SigningKey};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use tracing::instrument;

// On-disk JSON mirror of [`BucketPolicy`]. Kept separate so we can:
//   * strip the secret `SigningKey` wrapper to a raw byte sequence;
//   * avoid coupling storage format to any future additions on `BucketPolicy`
//     that shouldn't hit disk without an explicit opt-in.
// Field names/order match `Permission` exactly so the embedded `anonymous_access`
// round-trips as-is.
#[derive(Debug, Serialize, Deserialize)]
struct StoredBucketPolicy {
    secret_key_hash: Option<Vec<u8>>,
    read_key_hash: Option<Vec<u8>>,
    write_key_hash: Option<Vec<u8>>,
    signing_key: Option<Vec<u8>>,
    signing_key_generation: u64,
    default_ttl_secs: Option<u64>,
    anonymous_access: Permission,
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
            anonymous_access: p.anonymous_access,
        }
    }
}

impl From<StoredBucketPolicy> for BucketPolicy {
    fn from(s: StoredBucketPolicy) -> Self {
        Self {
            default_ttl_secs: s.default_ttl_secs,
            anonymous_access: s.anonymous_access,
            secret_key_hash: s.secret_key_hash,
            read_key_hash: s.read_key_hash,
            write_key_hash: s.write_key_hash,
            signing_key: s.signing_key.map(|b| SigningKey::new(Box::new(b))),
            signing_key_generation: s.signing_key_generation,
        }
    }
}

#[instrument(skip_all, name = "codec::encode_policy")]
pub fn encode_policy(policy: &BucketPolicy) -> Result<Vec<u8>, CodecError> {
    let stored = StoredBucketPolicy::from(policy);
    Ok(serde_json::to_vec(&stored)?)
}

#[instrument(skip_all, name = "codec::decode_policy")]
pub fn decode_policy(bytes: &[u8]) -> Result<BucketPolicy, CodecError> {
    let stored: StoredBucketPolicy = serde_json::from_slice(bytes)?;
    Ok(BucketPolicy::from(stored))
}

#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
}
