#![forbid(unsafe_code)]

//! Shared test fixtures and HTTP helpers.

mod buckets;
mod tokens;

use std::sync::{Arc, Mutex};

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STD;
use metrics_exporter_prometheus::{BuildError, PrometheusBuilder, PrometheusHandle};
use thiserror::Error;
use tracing::instrument;

pub use buckets::BucketSetup;
pub use tokens::{PolicyPatch, TokenSetup};

use keydock_domain::SigningKey;
use keydock_fjall::FjallStore;
use keydock_http::build_router;
use keydock_state::AppState;
use keydock_support::clock::SystemClock;

static PROMETHEUS: Mutex<Option<PrometheusHandle>> = Mutex::new(None);

#[derive(Debug, Error)]
enum TestKitError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Fjall(#[from] keydock_fjall::FjallError),

    #[error(transparent)]
    Metrics(#[from] BuildError),
}

fn prometheus_handle() -> Result<PrometheusHandle, TestKitError> {
    let mut guard = PROMETHEUS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(handle) = guard.as_ref() {
        return Ok(handle.clone());
    }
    let handle = PrometheusBuilder::new().install_recorder()?;
    *guard = Some(handle.clone());
    Ok(handle)
}

/// Full Axum test server with a temporary data directory.
///
/// The directory is dropped when `TestContext` is dropped.
pub struct TestContext {
    _dir: tempfile::TempDir,
    pub server: axum_test::TestServer,
}

impl TestContext {
    /// Builds a temporary data directory and full Axum app for integration tests.
    ///
    /// Panics if setup fails. The panic location points at the caller (test).
    #[track_caller]
    pub fn new() -> Self {
        match build_test_app() {
            Ok((dir, server)) => Self { _dir: dir, server },
            Err(e) => panic!("failed to construct test app: {e}"),
        }
    }

    /// Creates a bucket via `POST /` and returns the bucket id (body text).
    pub async fn create_bucket(&self, setup: BucketSetup) -> String {
        buckets::create_bucket(&self.server, &setup).await
    }
}

impl Default for TestContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Builds an HTTP `Authorization: Basic ...` header value for tests.
///
/// Uses `username:password` with password `ignored`, matching the legacy integration
/// test pattern for Basic auth against static keys.
pub fn basic_auth_header(credential: &str) -> String {
    let pair = format!("{credential}:ignored");
    let encoded = BASE64_STD.encode(pair.as_bytes());
    format!("Basic {encoded}")
}

/// JSON value matching the stable HTTP error body (`keydock_http::error::ErrorBody`) for assertions.
pub fn api_error_body_json(code: u16, message: &str) -> serde_json::Value {
    serde_json::json!({
        "error": {
            "code": code,
            "message": message
        }
    })
}

#[instrument(skip_all)]
fn build_test_app() -> Result<(tempfile::TempDir, axum_test::TestServer), TestKitError> {
    let dir = tempfile::tempdir()?;
    let store = Arc::new(FjallStore::open(dir.path())?);
    let buckets: Arc<dyn keydock_usecase::BucketRepository> = store.clone();
    let keys: Arc<dyn keydock_usecase::KeyRepository> = store.clone();
    let clock: Arc<dyn keydock_support::Clock> = Arc::new(SystemClock);

    let root_key = Arc::new(SigningKey::new(Box::new(
        b"test-root-key-32-bytes-minimum!!".to_vec(),
    )));
    let state = AppState::new("0.1.0-alpha", clock, buckets, keys, root_key);

    let prometheus = prometheus_handle()?;

    let router = build_router(state, prometheus.clone());
    let server = axum_test::TestServer::new(router);

    Ok((dir, server))
}
