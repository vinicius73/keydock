//! Bucket creation helpers for integration tests.

use axum_test::TestServer;
use bytes::Bytes;
use serde::Serialize;

/// Form payload for `POST /api/v1` (bucket creation) in integration tests.
#[derive(Clone, Debug, Serialize)]
pub struct BucketSetup {
    pub email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub write_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signing_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_ttl: Option<u64>,
}

impl Default for BucketSetup {
    fn default() -> Self {
        Self {
            email: "o@example.com".into(),
            secret_key: None,
            read_key: None,
            write_key: None,
            signing_key: None,
            default_ttl: None,
        }
    }
}

impl BucketSetup {
    /// Public bucket: no static credentials (anonymous read when applicable).
    pub fn public() -> Self {
        Self::default()
    }

    /// Admin via `secret_key` only.
    pub fn admin(secret: impl Into<String>) -> Self {
        Self {
            secret_key: Some(secret.into()),
            ..Self::default()
        }
    }

    /// Read-only static key.
    pub fn read_only(read: impl Into<String>) -> Self {
        Self {
            read_key: Some(read.into()),
            ..Self::default()
        }
    }

    /// Write-only static key.
    pub fn write_only(write: impl Into<String>) -> Self {
        Self {
            write_key: Some(write.into()),
            ..Self::default()
        }
    }

    /// Read + write static keys.
    pub fn restricted(read: impl Into<String>, write: impl Into<String>) -> Self {
        Self {
            read_key: Some(read.into()),
            write_key: Some(write.into()),
            ..Self::default()
        }
    }

    /// Secret + signing key (typical token test baseline).
    pub fn signed(secret: impl Into<String>, signing: impl Into<String>) -> Self {
        Self {
            secret_key: Some(secret.into()),
            signing_key: Some(signing.into()),
            ..Self::default()
        }
    }

    /// Signing key only; anonymous data-plane access stays public.
    pub fn signing_only(signing: impl Into<String>) -> Self {
        Self {
            signing_key: Some(signing.into()),
            ..Self::default()
        }
    }
}

/// Creates a bucket and returns its id (response body text).
pub async fn create_bucket(server: &TestServer, setup: &BucketSetup) -> String {
    let body = serde_urlencoded::to_string(setup).expect("encode bucket form");
    let response = server
        .post("/api/v1")
        .bytes(Bytes::from(body))
        .content_type("application/x-www-form-urlencoded")
        .await;
    response.assert_status_ok();
    response.text()
}
