//! Bucket-scoped authentication extractor (`FromRequestParts`).

use std::fmt;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::response::Response;
use bytes::Bytes;
use keydock_domain::{BucketId, Key, Permission};
use keydock_state::AppState;
use keydock_usecase::{ResolvedIdentity, resolve};
use tracing::instrument;

use crate::auth::{RawCredential, extract as extract_credential};
use crate::blocking;
use crate::error::{bad_request, forbidden, not_found, unauthorized};

/// Parses a percent-encoded path segment or JSON key string into a validated [`Key`].
pub(crate) fn parse_percent_encoded_key(raw: &str) -> Result<Key, Response> {
    let decoded: Vec<u8> = crate::percent::decode_to_bytes(raw);
    Key::from_bytes(Bytes::from(decoded)).map_err(|_| bad_request())
}

/// Which API key hashes are configured for the bucket (no secret material).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PolicyKeysPresence {
    pub has_read_key: bool,
    pub has_write_key: bool,
    pub has_secret_key: bool,
}

impl From<&keydock_domain::BucketPolicy> for PolicyKeysPresence {
    fn from(policy: &keydock_domain::BucketPolicy) -> Self {
        Self {
            has_read_key: policy.read_key_hash.is_some(),
            has_write_key: policy.write_key_hash.is_some(),
            has_secret_key: policy.secret_key_hash.is_some(),
        }
    }
}

/// Resolved bucket identity and policy metadata for handler permission checks.
#[derive(Clone)]
pub struct BucketAuth {
    pub identity: ResolvedIdentity,
    pub bucket_id: BucketId,
    pub policy_presence: PolicyKeysPresence,
    pub anonymous_access: Permission,
    pub default_ttl_secs: Option<u64>,
}

impl fmt::Debug for BucketAuth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BucketAuth")
            .field("identity", &"[redacted]")
            .field("bucket_id", &self.bucket_id)
            .field("policy_presence", &self.policy_presence)
            .field("anonymous_access", &self.anonymous_access)
            .finish()
    }
}

impl BucketAuth {
    #[instrument(skip_all, name = "BucketAuth::require_admin")]
    pub fn require_admin(&self) -> Result<(), Response> {
        match &self.identity {
            ResolvedIdentity::Admin => Ok(()),
            _ => {
                tracing::debug!(
                    bucket = %self.bucket_id.as_str(),
                    action = "admin",
                    identity_kind = %self.identity_kind(),
                    reason = "admin_required",
                    "authorization denied"
                );
                Err(forbidden())
            }
        }
    }

    #[instrument(skip_all, name = "BucketAuth::require_enumerate")]
    pub fn require_enumerate(&self) -> Result<(), Response> {
        match &self.identity {
            ResolvedIdentity::Admin => Ok(()),
            ResolvedIdentity::Scoped { permissions, .. } => {
                if permissions.enumerate {
                    Ok(())
                } else {
                    tracing::debug!(
                        bucket = %self.bucket_id.as_str(),
                        action = "enumerate",
                        identity_kind = "scoped",
                        reason = "missing_permission",
                        "authorization denied"
                    );
                    Err(forbidden())
                }
            }
            ResolvedIdentity::Anonymous => {
                if self.policy_presence.has_read_key {
                    tracing::debug!(
                        bucket = %self.bucket_id.as_str(),
                        action = "enumerate",
                        identity_kind = "anonymous",
                        reason = "credential_required",
                        "authorization denied"
                    );
                    Err(unauthorized())
                } else {
                    Ok(())
                }
            }
        }
    }

    #[instrument(skip_all, name = "BucketAuth::require_read_on")]
    pub fn require_read_on(&self, key: &Key) -> Result<(), Response> {
        let key_len = key.as_bytes().len();
        match &self.identity {
            ResolvedIdentity::Admin => Ok(()),
            ResolvedIdentity::Scoped {
                permissions,
                key_prefix,
            } => {
                if !permissions.read {
                    tracing::debug!(
                        bucket = %self.bucket_id.as_str(),
                        action = "read",
                        identity_kind = "scoped",
                        key_len,
                        reason = "missing_permission",
                        "authorization denied"
                    );
                    return Err(forbidden());
                }
                if !Self::is_prefix_ok(key_prefix, key) {
                    tracing::debug!(
                        bucket = %self.bucket_id.as_str(),
                        action = "read",
                        identity_kind = "scoped",
                        key_len,
                        prefix_len = key_prefix.len(),
                        reason = "prefix_mismatch",
                        "authorization denied"
                    );
                    return Err(forbidden());
                }
                Ok(())
            }
            ResolvedIdentity::Anonymous => {
                if self.policy_presence.has_read_key {
                    tracing::debug!(
                        bucket = %self.bucket_id.as_str(),
                        action = "read",
                        identity_kind = "anonymous",
                        key_len,
                        reason = "credential_required",
                        "authorization denied"
                    );
                    Err(unauthorized())
                } else {
                    Ok(())
                }
            }
        }
    }

    #[instrument(skip_all, name = "BucketAuth::require_write_on")]
    pub fn require_write_on(&self, key: &Key) -> Result<(), Response> {
        let key_len = key.as_bytes().len();
        match &self.identity {
            ResolvedIdentity::Admin => Ok(()),
            ResolvedIdentity::Scoped {
                permissions,
                key_prefix,
            } => {
                if !permissions.write {
                    tracing::debug!(
                        bucket = %self.bucket_id.as_str(),
                        action = "write",
                        identity_kind = "scoped",
                        key_len,
                        reason = "missing_permission",
                        "authorization denied"
                    );
                    return Err(forbidden());
                }
                if !Self::is_prefix_ok(key_prefix, key) {
                    tracing::debug!(
                        bucket = %self.bucket_id.as_str(),
                        action = "write",
                        identity_kind = "scoped",
                        key_len,
                        prefix_len = key_prefix.len(),
                        reason = "prefix_mismatch",
                        "authorization denied"
                    );
                    return Err(forbidden());
                }
                Ok(())
            }
            ResolvedIdentity::Anonymous => {
                if self.policy_presence.has_write_key {
                    tracing::debug!(
                        bucket = %self.bucket_id.as_str(),
                        action = "write",
                        identity_kind = "anonymous",
                        key_len,
                        reason = "credential_required",
                        "authorization denied"
                    );
                    Err(unauthorized())
                } else if self.anonymous_access.write {
                    Ok(())
                } else {
                    tracing::debug!(
                        bucket = %self.bucket_id.as_str(),
                        action = "write",
                        identity_kind = "anonymous",
                        key_len,
                        reason = "anonymous_forbidden",
                        "authorization denied"
                    );
                    Err(forbidden())
                }
            }
        }
    }

    #[instrument(skip_all, name = "BucketAuth::require_delete_on")]
    pub fn require_delete_on(&self, key: &Key) -> Result<(), Response> {
        let key_len = key.as_bytes().len();
        match &self.identity {
            ResolvedIdentity::Admin => Ok(()),
            ResolvedIdentity::Scoped {
                permissions,
                key_prefix,
            } => {
                if !permissions.delete {
                    tracing::debug!(
                        bucket = %self.bucket_id.as_str(),
                        action = "delete",
                        identity_kind = "scoped",
                        key_len,
                        reason = "missing_permission",
                        "authorization denied"
                    );
                    return Err(forbidden());
                }
                if !Self::is_prefix_ok(key_prefix, key) {
                    tracing::debug!(
                        bucket = %self.bucket_id.as_str(),
                        action = "delete",
                        identity_kind = "scoped",
                        key_len,
                        prefix_len = key_prefix.len(),
                        reason = "prefix_mismatch",
                        "authorization denied"
                    );
                    return Err(forbidden());
                }
                Ok(())
            }
            ResolvedIdentity::Anonymous => {
                if self.any_policy_key_present() {
                    tracing::debug!(
                        bucket = %self.bucket_id.as_str(),
                        action = "delete",
                        identity_kind = "anonymous",
                        key_len,
                        reason = "credential_required",
                        "authorization denied"
                    );
                    Err(unauthorized())
                } else if self.anonymous_access.delete {
                    Ok(())
                } else {
                    tracing::debug!(
                        bucket = %self.bucket_id.as_str(),
                        action = "delete",
                        identity_kind = "anonymous",
                        key_len,
                        reason = "anonymous_forbidden",
                        "authorization denied"
                    );
                    Err(forbidden())
                }
            }
        }
    }

    fn identity_kind(&self) -> &'static str {
        match &self.identity {
            ResolvedIdentity::Admin => "admin",
            ResolvedIdentity::Scoped { .. } => "scoped",
            ResolvedIdentity::Anonymous => "anonymous",
        }
    }

    fn any_policy_key_present(&self) -> bool {
        self.policy_presence.has_secret_key
            || self.policy_presence.has_write_key
            || self.policy_presence.has_read_key
    }

    fn is_prefix_ok(key_prefix: &[u8], key: &Key) -> bool {
        key_prefix.is_empty() || key.as_bytes().starts_with(key_prefix)
    }
}

fn first_path_segment(path: &str) -> Option<&str> {
    path.trim_start_matches('/')
        .split('/')
        .find(|segment| !segment.is_empty())
}

fn bucket_id_from_path(path: &str) -> Result<BucketId, ()> {
    let segment = first_path_segment(path).ok_or(())?;
    BucketId::new(segment.to_string()).map_err(|_| ())
}

impl FromRequestParts<AppState> for BucketAuth {
    type Rejection = Response;

    #[instrument(
        skip_all,
        name = "BucketAuth::from_request_parts",
        fields(bucket = tracing::field::Empty)
    )]
    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let path = parts.uri.path();
        let bucket_id = bucket_id_from_path(path).map_err(|()| bad_request())?;
        tracing::Span::current().record("bucket", bucket_id.as_str());

        let raw = extract_credential(&parts.headers, parts.uri.query());
        let cred_ref = raw.as_ref().map(RawCredential::as_str);

        let now = state.clock().now_utc();
        let buckets = state.buckets().clone();
        let bucket_id_for_policy = bucket_id.clone();
        let policy = blocking::spawn_usecase(move || buckets.get_policy(&bucket_id_for_policy))
            .await?
            .ok_or_else(not_found)?;

        let identity = resolve(
            cred_ref,
            &policy,
            &bucket_id,
            state.root_key().as_ref(),
            now,
        )
        .map_err(|_| {
            tracing::warn!(
                bucket = %bucket_id.as_str(),
                has_credential = raw.is_some(),
                "credential rejected"
            );
            unauthorized()
        })?;

        Ok(BucketAuth {
            identity,
            bucket_id,
            policy_presence: PolicyKeysPresence::from(&policy),
            anonymous_access: policy.anonymous_access,
            default_ttl_secs: policy.default_ttl_secs,
        })
    }
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use bytes::Bytes;
    use pretty_assertions::assert_eq;

    use keydock_domain::BucketId;

    use super::*;

    fn sample_key(name: &[u8]) -> Key {
        Key::from_bytes(Bytes::copy_from_slice(name)).expect("key")
    }

    fn auth(
        identity: ResolvedIdentity,
        presence: PolicyKeysPresence,
        anonymous: Permission,
    ) -> BucketAuth {
        BucketAuth {
            identity,
            bucket_id: BucketId::new("b".to_string()).expect("id"),
            policy_presence: presence,
            anonymous_access: anonymous,
            default_ttl_secs: None,
        }
    }

    #[test]
    fn require_read_admin_ok() {
        let a = auth(
            ResolvedIdentity::Admin,
            PolicyKeysPresence {
                has_read_key: true,
                has_write_key: false,
                has_secret_key: true,
            },
            Permission::NONE,
        );
        assert_eq!(a.require_read_on(&sample_key(b"k")).is_ok(), true);
    }

    #[test]
    fn require_read_scoped_empty_prefix_ok() {
        let a = auth(
            ResolvedIdentity::Scoped {
                permissions: Permission::READ_ONLY,
                key_prefix: Vec::new(),
            },
            PolicyKeysPresence {
                has_read_key: true,
                has_write_key: false,
                has_secret_key: false,
            },
            Permission::NONE,
        );
        assert_eq!(a.require_read_on(&sample_key(b"any")).is_ok(), true);
    }

    #[test]
    fn require_read_scoped_prefix_match_ok() {
        let a = auth(
            ResolvedIdentity::Scoped {
                permissions: Permission::READ_ONLY,
                key_prefix: b"user:42:".to_vec(),
            },
            PolicyKeysPresence {
                has_read_key: true,
                has_write_key: false,
                has_secret_key: false,
            },
            Permission::NONE,
        );
        assert_eq!(
            a.require_read_on(&sample_key(b"user:42:name")).is_ok(),
            true
        );
    }

    #[test]
    fn require_read_scoped_prefix_mismatch_forbidden() {
        let a = auth(
            ResolvedIdentity::Scoped {
                permissions: Permission::READ_ONLY,
                key_prefix: b"user:42:".to_vec(),
            },
            PolicyKeysPresence {
                has_read_key: true,
                has_write_key: false,
                has_secret_key: false,
            },
            Permission::NONE,
        );
        let err = a.require_read_on(&sample_key(b"admin:config")).unwrap_err();
        assert_eq!(err.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn require_read_anonymous_public_ok() {
        let a = auth(
            ResolvedIdentity::Anonymous,
            PolicyKeysPresence {
                has_read_key: false,
                has_write_key: false,
                has_secret_key: false,
            },
            Permission::NONE,
        );
        assert_eq!(a.require_read_on(&sample_key(b"k")).is_ok(), true);
    }

    #[test]
    fn require_read_anonymous_restricted_unauthorized() {
        let a = auth(
            ResolvedIdentity::Anonymous,
            PolicyKeysPresence {
                has_read_key: true,
                has_write_key: false,
                has_secret_key: false,
            },
            Permission::NONE,
        );
        let err = a.require_read_on(&sample_key(b"k")).unwrap_err();
        assert_eq!(err.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn require_enumerate_anonymous_no_read_key_ok() {
        let a = auth(
            ResolvedIdentity::Anonymous,
            PolicyKeysPresence {
                has_read_key: false,
                has_write_key: false,
                has_secret_key: false,
            },
            Permission::NONE,
        );
        assert_eq!(a.require_enumerate().is_ok(), true);
    }

    #[test]
    fn require_enumerate_anonymous_has_read_key_unauthorized() {
        let a = auth(
            ResolvedIdentity::Anonymous,
            PolicyKeysPresence {
                has_read_key: true,
                has_write_key: false,
                has_secret_key: false,
            },
            Permission::NONE,
        );
        let err = a.require_enumerate().unwrap_err();
        assert_eq!(err.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn require_write_anonymous_has_write_key_unauthorized() {
        let a = auth(
            ResolvedIdentity::Anonymous,
            PolicyKeysPresence {
                has_read_key: false,
                has_write_key: true,
                has_secret_key: false,
            },
            Permission::NONE,
        );
        let err = a.require_write_on(&sample_key(b"k")).unwrap_err();
        assert_eq!(err.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn require_write_anonymous_public_ok() {
        let a = auth(
            ResolvedIdentity::Anonymous,
            PolicyKeysPresence {
                has_read_key: false,
                has_write_key: false,
                has_secret_key: false,
            },
            Permission::anonymous_from_keys(false, false, false),
        );
        assert_eq!(a.require_write_on(&sample_key(b"k")).is_ok(), true);
    }

    #[test]
    fn require_delete_anonymous_any_key_present_unauthorized() {
        let a = auth(
            ResolvedIdentity::Anonymous,
            PolicyKeysPresence {
                has_read_key: true,
                has_write_key: false,
                has_secret_key: false,
            },
            Permission::anonymous_from_keys(false, true, false),
        );
        let err = a.require_delete_on(&sample_key(b"k")).unwrap_err();
        assert_eq!(err.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn require_delete_anonymous_no_keys_ok() {
        let a = auth(
            ResolvedIdentity::Anonymous,
            PolicyKeysPresence {
                has_read_key: false,
                has_write_key: false,
                has_secret_key: false,
            },
            Permission::anonymous_from_keys(false, false, false),
        );
        assert_eq!(a.require_delete_on(&sample_key(b"k")).is_ok(), true);
    }
}
