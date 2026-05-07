//! Token and policy helpers for integration tests.

use axum_test::TestServer;
use bytes::Bytes;
use keydock_domain::{BucketId, Permission, SigningKey, TemporaryTokenClaims};
use keydock_usecase::mint;
use serde::Serialize;
use time::{Duration, OffsetDateTime};

use crate::TestContext;

/// Form payload for `POST /api/v1{bucket}/tokens/`.
///
/// Both `prefix` and a positive `ttl` are mandatory at the server; these
/// fields carry the raw values that will be sent to the server (no filtering
/// or implicit defaults). Use the helpers below to build the common variants.
#[derive(Clone, Debug, Serialize)]
pub struct TokenSetup {
    /// Must be non-empty at the server side; tests that exercise the
    /// rejection path should set this to `""` explicitly.
    pub prefix: String,
    pub permissions: String,
    /// Seconds-from-now; must be strictly positive for a successful mint.
    pub ttl: i64,
}

impl TokenSetup {
    /// Read permission scoped to `prefix` with the given TTL (seconds).
    ///
    /// Pass an empty prefix to drive the 400 rejection path; all other
    /// callers should provide a concrete scope.
    pub fn read(prefix: impl Into<String>, ttl: i64) -> Self {
        Self {
            prefix: prefix.into(),
            permissions: "read".into(),
            ttl,
        }
    }
}

/// JSON body for `PATCH /api/v1/{bucket}`.
///
/// Uses `serde_json::Value` so tests can express the three-way semantics
/// (absent / `null` / value) without bespoke types: a field set to
/// `Value::Null` clears the corresponding policy entry on the server.
#[derive(Clone, Debug, Default, Serialize)]
pub struct PolicyPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_key: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_key: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub write_key: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signing_key: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_ttl: Option<serde_json::Value>,
}

impl PolicyPatch {
    /// Rotate signing key only (sets a new string value).
    pub fn rotate_signing_key(signing_key: impl Into<String>) -> Self {
        Self {
            signing_key: Some(serde_json::Value::String(signing_key.into())),
            ..Self::default()
        }
    }

    /// Clear the signing key (sends `"signing_key": null`).
    pub fn clear_signing_key() -> Self {
        Self {
            signing_key: Some(serde_json::Value::Null),
            ..Self::default()
        }
    }

    /// Clear the read key (sends `"read_key": null`).
    pub fn clear_read_key() -> Self {
        Self {
            read_key: Some(serde_json::Value::Null),
            ..Self::default()
        }
    }

    /// Clear the write key (sends `"write_key": null`).
    pub fn clear_write_key() -> Self {
        Self {
            write_key: Some(serde_json::Value::Null),
            ..Self::default()
        }
    }
}

impl TestContext {
    /// Creates a temporary access token for the bucket.
    pub async fn create_token(
        &self,
        bucket_id: &str,
        bearer: &str,
        setup: &TokenSetup,
    ) -> axum_test::TestResponse {
        create_token(&self.server, bucket_id, bearer, setup).await
    }

    /// Updates bucket policy (e.g. signing key rotation).
    pub async fn patch_policy(
        &self,
        bucket_id: &str,
        bearer: &str,
        patch: &PolicyPatch,
    ) -> axum_test::TestResponse {
        patch_policy(&self.server, bucket_id, bearer, patch).await
    }

    /// Mints a temporary token whose `exp` is already in the past,
    /// bypassing the HTTP mint endpoint (which now rejects `ttl <= 0`).
    ///
    /// Intended for negative tests that need to observe `verify` rejecting an
    /// expired token. `signing_secret` must match the bucket's configured
    /// signing key (typically the raw string passed to `BucketSetup`).
    pub fn mint_expired_read_token(&self, bucket_id: &str, signing_secret: &str) -> String {
        let bucket = BucketId::new(bucket_id.to_string()).expect("valid bucket id from API");
        let signing_key = SigningKey::new(Box::new(signing_secret.as_bytes().to_vec()));
        let now = OffsetDateTime::now_utc();
        let claims = TemporaryTokenClaims {
            version: 1,
            bucket,
            bucket_generation: 0,
            allowed_prefix: Vec::new(),
            permissions: Permission::READ_ONLY,
            iat: now - Duration::seconds(3600),
            exp: now - Duration::seconds(1),
        };
        mint(&claims, &signing_key).expect("mint expired token")
    }
}

pub async fn create_token(
    server: &TestServer,
    bucket_id: &str,
    bearer: &str,
    setup: &TokenSetup,
) -> axum_test::TestResponse {
    let body = serde_urlencoded::to_string(setup).expect("encode token form");
    server
        .post(&format!("/api/v1/{bucket_id}/tokens/"))
        .authorization_bearer(bearer)
        .bytes(Bytes::from(body))
        .content_type("application/x-www-form-urlencoded")
        .await
}

pub async fn patch_policy(
    server: &TestServer,
    bucket_id: &str,
    bearer: &str,
    patch: &PolicyPatch,
) -> axum_test::TestResponse {
    let body = serde_json::to_vec(patch).expect("encode policy patch as JSON");
    server
        .patch(&format!("/api/v1/{bucket_id}"))
        .authorization_bearer(bearer)
        .bytes(Bytes::from(body))
        .content_type("application/json")
        .await
}
