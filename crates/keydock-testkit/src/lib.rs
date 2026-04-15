#![forbid(unsafe_code)]

//! Shared test fixtures and HTTP helpers.

use std::sync::{Arc, Mutex};

use metrics_exporter_prometheus::{BuildError, PrometheusBuilder, PrometheusHandle};
use thiserror::Error;
use tracing::instrument;

use keydock_domain::SigningKey;
use keydock_fjall::FjallStore;
use keydock_http::build_router;
use keydock_state::AppState;
use keydock_support::clock::SystemClock;

static PROMETHEUS: Mutex<Option<PrometheusHandle>> = Mutex::new(None);

/// Errors while constructing the integration test app.
#[derive(Debug, Error)]
pub enum TestKitError {
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

/// Builds a temporary data directory and full Axum app for integration tests.
#[instrument(skip_all)]
pub fn test_app() -> Result<(tempfile::TempDir, axum_test::TestServer), TestKitError> {
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
