#![forbid(unsafe_code)]

//! Shared test fixtures and HTTP helpers.

use std::sync::{Arc, OnceLock};

use keydock_config::ValidatedHttpConfig;
use keydock_domain::SigningKey;
use keydock_fjall::FjallStore;
use keydock_http::build_router;
use keydock_state::AppState;
use keydock_support::clock::SystemClock;
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};

static PROMETHEUS: OnceLock<PrometheusHandle> = OnceLock::new();

fn prometheus_handle() -> PrometheusHandle {
    PROMETHEUS
        .get_or_init(|| {
            PrometheusBuilder::new()
                .install_recorder()
                .expect("prometheus recorder")
        })
        .clone()
}

/// Builds a temporary data directory and full Axum app for integration tests.
pub fn test_app() -> (tempfile::TempDir, axum_test::TestServer) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Arc::new(FjallStore::open(dir.path()).expect("fjall"));
    let buckets: Arc<dyn keydock_usecase::BucketRepository> = store.clone();
    let keys: Arc<dyn keydock_usecase::KeyRepository> = store.clone();
    let clock: Arc<dyn keydock_support::Clock> = Arc::new(SystemClock);

    let http = ValidatedHttpConfig {
        listen: "127.0.0.1:0".parse().expect("parse"),
        metrics_listen: None,
        log_json: false,
    };

    let root_key = Arc::new(SigningKey::new(Box::new(
        b"test-root-key-32-bytes-minimum!!".to_vec(),
    )));
    let state = AppState::new(http, "0.1.0-alpha", clock, buckets, keys, root_key);

    let prometheus = prometheus_handle();

    let router = build_router(state, prometheus.clone());
    let server = axum_test::TestServer::new(router);

    (dir, server)
}
