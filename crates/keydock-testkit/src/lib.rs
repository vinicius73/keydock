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

use keydock_domain::{BucketId, Key, SigningKey};
use keydock_fjall::FjallStore;
pub use keydock_http::{RateLimitSettings, RouterOptions};
use keydock_http::{build_router, init_rate_limiter};
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

// The Prometheus recorder is a process-global singleton; attempting to
// install it twice panics. Serialize initialization under `PROMETHEUS` so
// concurrent tests share a single handle, and register metric descriptions
// in the same critical section to keep them bound to that handle.
fn prometheus_handle() -> Result<PrometheusHandle, TestKitError> {
    let mut guard = PROMETHEUS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(handle) = guard.as_ref() {
        return Ok(handle.clone());
    }
    let handle = PrometheusBuilder::new().install_recorder()?;
    keydock_http::describe_all();
    *guard = Some(handle.clone());
    Ok(handle)
}

/// Full Axum test server with a temporary data directory.
///
/// The directory is dropped when `TestContext` is dropped.
pub struct TestContext {
    _dir: tempfile::TempDir,
    store: Arc<FjallStore>,
    pub server: axum_test::TestServer,
}

impl TestContext {
    /// Builds a temporary data directory and full Axum app for integration tests.
    ///
    /// Panics if setup fails. The panic location points at the caller (test).
    #[track_caller]
    pub fn new() -> Self {
        match build_test_app(RouterOptions::default()) {
            Ok((dir, store, server)) => Self {
                _dir: dir,
                store,
                server,
            },
            Err(e) => panic!("failed to construct test app: {e}"),
        }
    }

    /// Like [`Self::new`], but uses custom router wiring (e.g. rate limiting).
    ///
    /// When `opts.rate_limit.enabled` is true, initializes the process-global limiter
    /// before building the router; tests that enable this **must** run serially
    /// (see integration tests using [`serial_test::serial`]).
    #[instrument(
        skip_all,
        fields(
            expose_metrics = opts.expose_metrics,
            rate_limit_enabled = opts.rate_limit.enabled,
            rate_limit_requests_per_hour = opts.rate_limit.requests_per_hour
        )
    )]
    pub async fn with_router_options(opts: RouterOptions) -> Self {
        if opts.rate_limit.enabled {
            init_rate_limiter(&opts.rate_limit).await;
        }
        match build_test_app(opts) {
            Ok((dir, store, server)) => Self {
                _dir: dir,
                store,
                server,
            },
            Err(e) => panic!("failed to construct test app: {e}"),
        }
    }

    /// Simulates storage metadata failure for [`GET /ready`](crate) readiness checks.
    #[instrument(skip_all, fields(fail_ping_metadata = fail))]
    pub fn testkit_set_fail_ping_metadata(&self, fail: bool) {
        self.store.testkit_set_fail_ping_metadata(fail);
    }

    /// Creates a bucket via `POST /api/v1` and returns the bucket id (body text).
    pub async fn create_bucket(&self, setup: BucketSetup) -> String {
        buckets::create_bucket(&self.server, &setup).await
    }

    /// Scrapes `GET /metrics` through the live router, returning the body.
    /// Exercises the full wiring (router → handler → recorder render) rather
    /// than reaching into the Prometheus handle directly.
    pub async fn render_metrics(&self) -> String {
        self.server.get("/metrics").await.text()
    }

    /// Overwrites the entry at `(bucket, key)` with bytes that fail `postcard`
    /// decoding. The next read triggers `FjallError::Codec`, which the
    /// metrics smoke test observes as
    /// `storage_errors_total{kind="codec_entry"}`.
    pub fn corrupt_entry(&self, bucket: &BucketId, key: &Key) {
        self.store
            .testkit_write_raw(bucket, key, &[0xFF, 0xFF, 0xFF, 0xFF])
            .expect("inject corrupted entry bytes");
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
fn build_test_app(
    router_options: RouterOptions,
) -> Result<(tempfile::TempDir, Arc<FjallStore>, axum_test::TestServer), TestKitError> {
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

    let router = build_router(state, prometheus.clone(), router_options);
    let server = axum_test::TestServer::new(router);

    Ok((dir, store, server))
}
