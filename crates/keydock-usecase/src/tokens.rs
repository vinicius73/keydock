//! HMAC-SHA256 signed temporary bucket tokens (non-JWT).

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hmac::Mac;
use hmac::digest::KeyInit;
use keydock_domain::{BucketId, BucketPolicy, SigningKey, TemporaryTokenClaims};
use secrecy::ExposeSecret;
use sha2::Sha256;
use time::OffsetDateTime;

use crate::TokenError;

type HmacSha256 = hmac::Hmac<Sha256>;

/// Signs JSON claims with the bucket `signing_key`. Format: `base64url(json).base64url(sig)`.
#[tracing::instrument(skip_all)]
pub fn mint(claims: &TemporaryTokenClaims, signing_key: &SigningKey) -> Result<String, TokenError> {
    let json = serde_json::to_vec(claims).map_err(|_| TokenError::Serialize)?;
    let sig = hmac_sign(signing_key.expose_secret(), &json)?;
    let payload_b64 = URL_SAFE_NO_PAD.encode(&json);
    let sig_b64 = URL_SAFE_NO_PAD.encode(sig);
    Ok(format!("{payload_b64}.{sig_b64}"))
}

/// Verifies token signature and claims against policy and bucket.
#[tracing::instrument(skip_all)]
pub fn verify(
    raw: &str,
    policy: &BucketPolicy,
    bucket: &BucketId,
    now: OffsetDateTime,
) -> Result<TemporaryTokenClaims, TokenError> {
    let signing_key = policy
        .signing_key
        .as_ref()
        .ok_or(TokenError::NoSigningKey)?;

    let (payload_b64, sig_b64) = raw.split_once('.').ok_or(TokenError::InvalidFormat)?;
    let json = URL_SAFE_NO_PAD
        .decode(payload_b64)
        .map_err(|_| TokenError::InvalidFormat)?;
    let expected_sig = URL_SAFE_NO_PAD
        .decode(sig_b64)
        .map_err(|_| TokenError::InvalidFormat)?;

    let computed = hmac_sign(signing_key.expose_secret(), &json)?;
    if computed.len() != expected_sig.len() || !crate::ct::eq_bytes(&computed, &expected_sig) {
        return Err(TokenError::InvalidSignature);
    }

    let claims: TemporaryTokenClaims =
        serde_json::from_slice(&json).map_err(|_| TokenError::InvalidFormat)?;

    if claims.exp <= now {
        return Err(TokenError::Expired);
    }
    if claims.bucket != *bucket {
        return Err(TokenError::BucketMismatch);
    }
    if claims.bucket_generation != policy.signing_key_generation {
        return Err(TokenError::GenerationMismatch);
    }

    Ok(claims)
}

fn hmac_sign(key: &[u8], data: &[u8]) -> Result<Vec<u8>, TokenError> {
    let mut mac = HmacSha256::new_from_slice(key).map_err(|_| TokenError::InvalidFormat)?;
    mac.update(data);
    Ok(mac.finalize().into_bytes().to_vec())
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use keydock_domain::Permission;

    use super::*;

    fn test_signing_key() -> SigningKey {
        SigningKey::new(Box::new(b"test-signing-key-bytes!!".to_vec()))
    }

    fn sample_claims(bucket: BucketId, generation: u64) -> TemporaryTokenClaims {
        TemporaryTokenClaims {
            version: 1,
            bucket,
            bucket_generation: generation,
            allowed_prefix: b"user:".to_vec(),
            permissions: Permission::READ_ONLY,
            iat: OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
            exp: OffsetDateTime::from_unix_timestamp(2_000_000_000).unwrap(),
        }
    }

    fn policy_with_key(generation: u64) -> BucketPolicy {
        BucketPolicy {
            default_ttl_secs: None,
            anonymous_access: Permission::NONE,
            secret_key_hash: None,
            read_key_hash: None,
            write_key_hash: None,
            signing_key: Some(test_signing_key()),
            signing_key_generation: generation,
        }
    }

    #[derive(Clone, Copy)]
    enum VerifyRejectionCase {
        TamperedSignature,
        Expired,
        WrongBucket,
        GenerationMismatch,
        NoSigningKey,
    }

    impl VerifyRejectionCase {
        fn expected(self) -> TokenError {
            match self {
                Self::TamperedSignature => TokenError::InvalidSignature,
                Self::Expired => TokenError::Expired,
                Self::WrongBucket => TokenError::BucketMismatch,
                Self::GenerationMismatch => TokenError::GenerationMismatch,
                Self::NoSigningKey => TokenError::NoSigningKey,
            }
        }

        fn run(self) -> TokenError {
            let bucket = BucketId::new("b1".to_string()).unwrap();
            match self {
                Self::NoSigningKey => {
                    let mut policy = policy_with_key(0);
                    policy.signing_key = None;
                    verify("x.y", &policy, &bucket, OffsetDateTime::now_utc()).unwrap_err()
                }
                _ => {
                    let claims = sample_claims(bucket.clone(), 0);
                    let raw = mint(&claims, &test_signing_key()).unwrap();
                    let policy = policy_with_key(0);
                    let now_ok = OffsetDateTime::from_unix_timestamp(1_800_000_000).unwrap();
                    match self {
                        Self::TamperedSignature => {
                            let (payload_b64, _) = raw.split_once('.').unwrap();
                            let wrong_sig = URL_SAFE_NO_PAD.encode([0u8; 32]);
                            let tampered = format!("{payload_b64}.{wrong_sig}");
                            verify(&tampered, &policy, &bucket, now_ok).unwrap_err()
                        }
                        Self::Expired => {
                            let now = OffsetDateTime::from_unix_timestamp(2_100_000_000).unwrap();
                            verify(&raw, &policy, &bucket, now).unwrap_err()
                        }
                        Self::WrongBucket => {
                            let other = BucketId::new("b2".to_string()).unwrap();
                            verify(&raw, &policy, &other, now_ok).unwrap_err()
                        }
                        Self::GenerationMismatch => {
                            let mut p = policy_with_key(0);
                            p.signing_key_generation = 1;
                            verify(&raw, &p, &bucket, now_ok).unwrap_err()
                        }
                        Self::NoSigningKey => unreachable!(),
                    }
                }
            }
        }
    }

    #[test]
    fn mint_verify_roundtrip() {
        let bucket = BucketId::new("b1".to_string()).unwrap();
        let claims = sample_claims(bucket.clone(), 0);
        let key = test_signing_key();
        let raw = mint(&claims, &key).unwrap();
        let policy = policy_with_key(0);
        let now = OffsetDateTime::from_unix_timestamp(1_800_000_000).unwrap();
        let out = verify(&raw, &policy, &bucket, now).unwrap();
        assert_eq!(out.bucket, claims.bucket);
        assert_eq!(out.bucket_generation, claims.bucket_generation);
        assert_eq!(out.allowed_prefix, claims.allowed_prefix);
        assert_eq!(out.permissions, claims.permissions);
    }

    #[rstest]
    #[case::tampered(VerifyRejectionCase::TamperedSignature)]
    #[case::expired(VerifyRejectionCase::Expired)]
    #[case::wrong_bucket(VerifyRejectionCase::WrongBucket)]
    #[case::generation_mismatch(VerifyRejectionCase::GenerationMismatch)]
    #[case::no_signing_key(VerifyRejectionCase::NoSigningKey)]
    fn verify_rejects_invalid_tokens(#[case] case: VerifyRejectionCase) {
        let actual = case.run();
        assert_eq!(actual, case.expected());
    }
}
