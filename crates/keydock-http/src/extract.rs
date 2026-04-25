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
            _ => Err(forbidden()),
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
                    Err(forbidden())
                }
            }
            ResolvedIdentity::Anonymous => {
                if self.policy_presence.has_read_key {
                    Err(unauthorized())
                } else {
                    Ok(())
                }
            }
        }
    }

    #[instrument(skip_all, name = "BucketAuth::require_read_on")]
    pub fn require_read_on(&self, key: &Key) -> Result<(), Response> {
        match &self.identity {
            ResolvedIdentity::Admin => Ok(()),
            ResolvedIdentity::Scoped {
                permissions,
                key_prefix,
            } => {
                if !permissions.read {
                    return Err(forbidden());
                }
                Self::enforce_prefix(key_prefix, key)?;
                Ok(())
            }
            ResolvedIdentity::Anonymous => {
                if self.policy_presence.has_read_key {
                    Err(unauthorized())
                } else {
                    Ok(())
                }
            }
        }
    }

    #[instrument(skip_all, name = "BucketAuth::require_write_on")]
    pub fn require_write_on(&self, key: &Key) -> Result<(), Response> {
        match &self.identity {
            ResolvedIdentity::Admin => Ok(()),
            ResolvedIdentity::Scoped {
                permissions,
                key_prefix,
            } => {
                if !permissions.write {
                    return Err(forbidden());
                }
                Self::enforce_prefix(key_prefix, key)?;
                Ok(())
            }
            ResolvedIdentity::Anonymous => {
                if self.policy_presence.has_write_key {
                    Err(unauthorized())
                } else if self.anonymous_access.write {
                    Ok(())
                } else {
                    Err(forbidden())
                }
            }
        }
    }

    #[instrument(skip_all, name = "BucketAuth::require_delete_on")]
    pub fn require_delete_on(&self, key: &Key) -> Result<(), Response> {
        match &self.identity {
            ResolvedIdentity::Admin => Ok(()),
            ResolvedIdentity::Scoped {
                permissions,
                key_prefix,
            } => {
                if !permissions.delete {
                    return Err(forbidden());
                }
                Self::enforce_prefix(key_prefix, key)?;
                Ok(())
            }
            ResolvedIdentity::Anonymous => {
                if self.any_policy_key_present() {
                    Err(unauthorized())
                } else if self.anonymous_access.delete {
                    Ok(())
                } else {
                    Err(forbidden())
                }
            }
        }
    }

    fn any_policy_key_present(&self) -> bool {
        self.policy_presence.has_secret_key
            || self.policy_presence.has_write_key
            || self.policy_presence.has_read_key
    }

    fn enforce_prefix(key_prefix: &[u8], key: &Key) -> Result<(), Response> {
        if key_prefix.is_empty() {
            return Ok(());
        }
        if key.as_bytes().starts_with(key_prefix) {
            Ok(())
        } else {
            Err(forbidden())
        }
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

        let identity = match resolve(
            cred_ref,
            &policy,
            &bucket_id,
            state.root_key().as_ref(),
            now,
        ) {
            Ok(id) => id,
            Err(_) => return Err(unauthorized()),
        };

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
    use keydock_domain::BucketId;

    use super::*;
    use pretty_assertions::assert_eq;

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
}
