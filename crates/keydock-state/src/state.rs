use std::sync::Arc;

use axum::extract::FromRef;
use keydock_config::ValidatedHttpConfig;
use keydock_domain::SigningKey;
use keydock_support::Clock;
use keydock_usecase::ports::{BucketRepository, KeyRepository};

/// Axum-facing aggregate state (handlers stay thin; business rules live in use cases).
#[derive(Clone)]
pub struct AppState {
    pub http: ValidatedHttpConfig,
    pub version: &'static str,
    pub clock: Arc<dyn Clock>,
    pub buckets: Arc<dyn BucketRepository>,
    pub keys: Arc<dyn KeyRepository>,
    /// Root key for HMAC hashing of API credentials (`secret_key` / `read_key` / `write_key`).
    pub root_key: Arc<SigningKey>,
}

impl AppState {
    pub fn new(
        http: ValidatedHttpConfig,
        version: &'static str,
        clock: Arc<dyn Clock>,
        buckets: Arc<dyn BucketRepository>,
        keys: Arc<dyn KeyRepository>,
        root_key: Arc<SigningKey>,
    ) -> Self {
        Self {
            http,
            version,
            clock,
            buckets,
            keys,
            root_key,
        }
    }
}

impl FromRef<AppState> for Arc<dyn KeyRepository> {
    fn from_ref(state: &AppState) -> Self {
        state.keys.clone()
    }
}

impl FromRef<AppState> for Arc<dyn BucketRepository> {
    fn from_ref(state: &AppState) -> Self {
        state.buckets.clone()
    }
}

impl FromRef<AppState> for Arc<dyn Clock> {
    fn from_ref(state: &AppState) -> Self {
        state.clock.clone()
    }
}

impl FromRef<AppState> for Arc<SigningKey> {
    fn from_ref(state: &AppState) -> Self {
        state.root_key.clone()
    }
}
