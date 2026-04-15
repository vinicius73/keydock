//! Token and policy helpers for integration tests.

use axum_test::TestServer;
use bytes::Bytes;
use serde::Serialize;

use crate::TestContext;

/// Form payload for `POST /{bucket}/tokens/`.
#[derive(Clone, Debug, Serialize)]
pub struct TokenSetup {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    pub permissions: String,
    pub ttl: i64,
}

impl TokenSetup {
    /// Read permission with the given TTL (seconds).
    pub fn read(ttl: i64) -> Self {
        Self {
            prefix: None,
            permissions: "read".into(),
            ttl,
        }
    }

    /// Read permission scoped to a key prefix.
    pub fn read_prefixed(prefix: impl Into<String>, ttl: i64) -> Self {
        Self {
            prefix: Some(prefix.into()),
            permissions: "read".into(),
            ttl,
        }
    }

    /// Expired token (TTL zero) for negative tests.
    pub fn expired() -> Self {
        Self {
            prefix: None,
            permissions: "read".into(),
            ttl: 0,
        }
    }
}

/// Form payload for `PATCH /{bucket}` (policy updates).
#[derive(Clone, Debug, Serialize)]
pub struct PolicyPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signing_key: Option<String>,
}

impl PolicyPatch {
    /// Rotate signing key only.
    pub fn rotate_signing_key(signing_key: impl Into<String>) -> Self {
        Self {
            signing_key: Some(signing_key.into()),
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
}

pub async fn create_token(
    server: &TestServer,
    bucket_id: &str,
    bearer: &str,
    setup: &TokenSetup,
) -> axum_test::TestResponse {
    let body = serde_urlencoded::to_string(setup).expect("encode token form");
    server
        .post(&format!("/{bucket_id}/tokens/"))
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
    let body = serde_urlencoded::to_string(patch).expect("encode policy patch");
    server
        .patch(&format!("/{bucket_id}"))
        .authorization_bearer(bearer)
        .bytes(Bytes::from(body))
        .content_type("application/x-www-form-urlencoded")
        .await
}
