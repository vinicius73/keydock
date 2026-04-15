use secrecy::ExposeSecret;

use crate::permission::Permission;
use crate::token::SigningKey;

/// Policy attached to a bucket: hashed API keys, signing key for tokens, and anonymous defaults.
///
/// `secret_key_hash` / `read_key_hash` / `write_key_hash` store `HMAC-SHA256(root_key, raw_credential)` bytes.
/// `signing_key` holds raw key material for minting and verifying temporary tokens (never a client credential).
#[derive(Debug)]
pub struct BucketPolicy {
    pub default_ttl_secs: Option<u64>,
    pub anonymous_access: Permission,
    pub secret_key_hash: Option<Vec<u8>>,
    pub read_key_hash: Option<Vec<u8>>,
    pub write_key_hash: Option<Vec<u8>>,
    pub signing_key: Option<SigningKey>,
    /// Bumps when the signing key rotates so outstanding tokens can be rejected.
    pub signing_key_generation: u64,
}

// `SigningKey` is `SecretBox<Vec<u8>>`; `Vec<u8>` is not `CloneableSecret`, so `SecretBox` does not implement `Clone`.
impl Clone for BucketPolicy {
    fn clone(&self) -> Self {
        Self {
            default_ttl_secs: self.default_ttl_secs,
            anonymous_access: self.anonymous_access,
            secret_key_hash: self.secret_key_hash.clone(),
            read_key_hash: self.read_key_hash.clone(),
            write_key_hash: self.write_key_hash.clone(),
            signing_key: self
                .signing_key
                .as_ref()
                .map(|k| SigningKey::new(Box::new(k.expose_secret().clone()))),
            signing_key_generation: self.signing_key_generation,
        }
    }
}
