//! Credential resolution: direct key hashes and HMAC-signed temporary tokens.

use keydock_domain::{BucketId, BucketPolicy, Permission, SigningKey};
use secrecy::ExposeSecret;
use time::OffsetDateTime;

use crate::crypto;
use crate::{AuthError, tokens};

/// Identity after resolving HTTP credentials against a bucket policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedIdentity {
    Anonymous,
    Admin,
    Scoped {
        permissions: Permission,
        key_prefix: Vec<u8>,
    },
}

/// Stores `HMAC-SHA256(root_key, raw_credential_bytes)` as bytes.
#[tracing::instrument(skip_all)]
pub fn hash_credential(raw: &str, root_key: &SigningKey) -> Result<Vec<u8>, AuthError> {
    crypto::hmac_sha256(root_key.expose_secret(), raw.as_bytes())
        .map_err(|crypto::HmacError::InvalidKey| AuthError::InvalidKeyMaterial)
}

/// Constant-time compare of presented credential against stored hash.
#[tracing::instrument(skip_all)]
pub fn verify_credential(presented: &str, stored_hash: &[u8], root_key: &SigningKey) -> bool {
    let Ok(computed) = hash_credential(presented, root_key) else {
        return false;
    };
    computed.len() == stored_hash.len() && crate::ct::eq_bytes(&computed, stored_hash)
}

/// Resolves `raw` (Bearer/Basic/query string value) or anonymous access.
///
/// Order: `secret_key_hash` → `write_key_hash` → `read_key_hash` → signed token.
/// `signing_key` is never matched as a direct credential string.
#[tracing::instrument(skip_all, fields(bucket = %bucket.as_str()))]
pub fn resolve(
    raw: Option<&str>,
    policy: &BucketPolicy,
    bucket: &BucketId,
    root_key: &SigningKey,
    now: OffsetDateTime,
) -> Result<ResolvedIdentity, AuthError> {
    let Some(cred) = raw else {
        return Ok(ResolvedIdentity::Anonymous);
    };

    if let Some(ref h) = policy.secret_key_hash
        && verify_credential(cred, h, root_key)
    {
        return Ok(ResolvedIdentity::Admin);
    }
    if let Some(ref h) = policy.write_key_hash
        && verify_credential(cred, h, root_key)
    {
        return Ok(ResolvedIdentity::Scoped {
            permissions: Permission::WRITE_ONLY,
            key_prefix: Vec::new(),
        });
    }
    if let Some(ref h) = policy.read_key_hash
        && verify_credential(cred, h, root_key)
    {
        // `read_key` authenticates the bucket's read side, which covers both
        // `get` and `list`. Granting `enumerate` here preserves the
        // "public read with a known key" flow without forcing clients onto
        // `secret_key`.
        return Ok(ResolvedIdentity::Scoped {
            permissions: Permission::READ_ENUMERATE,
            key_prefix: Vec::new(),
        });
    }

    let claims =
        tokens::verify(cred, policy, bucket, now).map_err(|_| AuthError::InvalidCredential)?;
    Ok(ResolvedIdentity::Scoped {
        permissions: claims.permissions,
        key_prefix: claims.allowed_prefix,
    })
}

#[cfg(test)]
mod tests {
    use keydock_domain::BucketPolicy;

    use super::*;
    use pretty_assertions::assert_eq;

    fn root_key() -> SigningKey {
        SigningKey::new(Box::new(b"root-key-test-32-bytes-min!!".to_vec()))
    }

    fn policy_with_hashes(
        secret: Option<&str>,
        write: Option<&str>,
        read: Option<&str>,
    ) -> BucketPolicy {
        let rk = root_key();
        BucketPolicy {
            default_ttl_secs: None,
            anonymous_access: Permission::NONE,
            secret_key_hash: secret.map(|s| hash_credential(s, &rk).unwrap()),
            write_key_hash: write.map(|s| hash_credential(s, &rk).unwrap()),
            read_key_hash: read.map(|s| hash_credential(s, &rk).unwrap()),
            signing_key: None,
            signing_key_generation: 0,
        }
    }

    #[test]
    fn hash_verify_roundtrip() {
        let rk = root_key();
        let h = hash_credential("my-secret", &rk).unwrap();
        assert_eq!(verify_credential("my-secret", &h, &rk), true);
        assert_eq!(verify_credential("wrong", &h, &rk), false);
    }

    #[test]
    fn resolve_admin() {
        let bucket = BucketId::new("b".to_string()).unwrap();
        let p = policy_with_hashes(Some("adm"), None, None);
        let rk = root_key();
        let now = OffsetDateTime::now_utc();
        let id = resolve(Some("adm"), &p, &bucket, &rk, now).unwrap();
        assert_eq!(id, ResolvedIdentity::Admin);
    }

    #[test]
    fn resolve_write_scoped() {
        let bucket = BucketId::new("b".to_string()).unwrap();
        let p = policy_with_hashes(None, Some("w"), None);
        let rk = root_key();
        let now = OffsetDateTime::now_utc();
        let id = resolve(Some("w"), &p, &bucket, &rk, now).unwrap();
        if let ResolvedIdentity::Scoped {
            permissions,
            key_prefix,
        } = id
        {
            assert_eq!(permissions, Permission::WRITE_ONLY);
            assert_eq!(key_prefix.len(), 0);
        } else {
            assert_eq!(false, true, "expected Scoped identity");
        }
    }

    #[test]
    fn resolve_read_key_grants_read_and_enumerate() {
        let bucket = BucketId::new("b".to_string()).unwrap();
        let p = policy_with_hashes(None, None, Some("r"));
        let rk = root_key();
        let now = OffsetDateTime::now_utc();
        let id = resolve(Some("r"), &p, &bucket, &rk, now).unwrap();
        if let ResolvedIdentity::Scoped {
            permissions,
            key_prefix,
        } = id
        {
            assert_eq!(permissions, Permission::READ_ENUMERATE);
            assert_eq!(key_prefix.len(), 0);
        } else {
            assert_eq!(false, true, "expected Scoped identity");
        }
    }

    #[test]
    fn resolve_anonymous() {
        let bucket = BucketId::new("b".to_string()).unwrap();
        let p = policy_with_hashes(None, None, None);
        let rk = root_key();
        let now = OffsetDateTime::now_utc();
        let id = resolve(None, &p, &bucket, &rk, now).unwrap();
        assert_eq!(id, ResolvedIdentity::Anonymous);
    }

    #[test]
    fn resolve_invalid_returns_err() {
        let bucket = BucketId::new("b".to_string()).unwrap();
        let p = policy_with_hashes(Some("adm"), None, None);
        let rk = root_key();
        let now = OffsetDateTime::now_utc();
        let err = resolve(Some("nope"), &p, &bucket, &rk, now).unwrap_err();
        assert_eq!(
            matches!(err, AuthError::InvalidCredential),
            true,
            "expected InvalidCredential"
        );
    }
}
