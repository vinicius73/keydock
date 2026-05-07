use std::sync::Arc;

use axum::extract::FromRef;

use keydock_domain::SigningKey;
use keydock_support::Clock;
use keydock_usecase::ports::{BucketRepository, KeyRepository};

/// Axum-facing aggregate state (handlers stay thin; business rules live in use cases).
#[derive(Clone)]
pub struct AppState {
    version: &'static str,
    clock: Arc<dyn Clock>,
    buckets: Arc<dyn BucketRepository>,
    keys: Arc<dyn KeyRepository>,
    /// Root key for HMAC hashing of API credentials (`secret_key` / `read_key` / `write_key`).
    root_key: Arc<SigningKey>,
}

impl AppState {
    #[tracing::instrument(skip_all, name = "AppState::new")]
    pub fn new(
        version: &'static str,
        clock: Arc<dyn Clock>,
        buckets: Arc<dyn BucketRepository>,
        keys: Arc<dyn KeyRepository>,
        root_key: Arc<SigningKey>,
    ) -> Self {
        Self {
            version,
            clock,
            buckets,
            keys,
            root_key,
        }
    }

    pub fn version(&self) -> &'static str {
        self.version
    }

    pub fn clock(&self) -> &Arc<dyn Clock> {
        &self.clock
    }

    pub fn buckets(&self) -> &Arc<dyn BucketRepository> {
        &self.buckets
    }

    pub fn keys(&self) -> &Arc<dyn KeyRepository> {
        &self.keys
    }

    pub fn root_key(&self) -> &Arc<SigningKey> {
        &self.root_key
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
