use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use keydock_state::AppState;
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct HealthResponse {
    pub status: &'static str,
    pub storage: &'static str,
    pub version: &'static str,
}

/// Liveness probe (no dependencies; does not check storage).
#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "OK", body = HealthResponse)
    )
)]
pub async fn health_check(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        storage: "ok",
        version: state.version(),
    })
}

/// Readiness probe: verifies metadata storage is reachable.
#[utoipa::path(
    get,
    path = "/ready",
    responses(
        (status = 200, description = "Storage OK", body = HealthResponse),
        (status = 503, description = "Storage unavailable", body = HealthResponse)
    )
)]
pub async fn readiness_check(State(state): State<AppState>) -> Response {
    let buckets = state.buckets().clone();
    let ok = match tokio::task::spawn_blocking(move || buckets.ping_metadata()).await {
        Ok(Ok(())) => true,
        Ok(Err(e)) => {
            tracing::debug!(error = %e, "readiness: storage ping failed");
            false
        }
        Err(e) => {
            tracing::warn!(error = %e, "readiness: spawn_blocking join failed");
            false
        }
    };

    match ok {
        true => (
            StatusCode::OK,
            Json(HealthResponse {
                status: "ok",
                storage: "ok",
                version: state.version(),
            }),
        )
            .into_response(),
        false => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(HealthResponse {
                status: "degraded",
                storage: "error",
                version: state.version(),
            }),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::extract::State;
    use http_body_util::BodyExt;
    use keydock_domain::{BucketId, BucketPolicy, CounterOp, Key, SigningKey, StoredValue};
    use keydock_support::clock::SystemClock;
    use keydock_usecase::ports::{BucketRepository, KeyRepository};
    use keydock_usecase::{ListEntry, ListOpts, StoredEntry, TxnOp, UseCaseError};
    use pretty_assertions::assert_eq;
    use time::OffsetDateTime;

    use super::*;

    struct AlwaysFailingBuckets;

    impl BucketRepository for AlwaysFailingBuckets {
        fn ping_metadata(&self) -> Result<(), UseCaseError> {
            Err(UseCaseError::Storage("ping failed".into()))
        }

        fn get_policy(&self, _bucket: &BucketId) -> Result<Option<BucketPolicy>, UseCaseError> {
            Ok(None)
        }

        fn create_bucket(&self, _id: &BucketId, _policy: BucketPolicy) -> Result<(), UseCaseError> {
            Ok(())
        }

        fn delete_bucket(&self, _id: &BucketId) -> Result<(), UseCaseError> {
            Ok(())
        }
    }

    struct NoopKeys;

    impl KeyRepository for NoopKeys {
        fn get(&self, _bucket: &BucketId, _key: &Key) -> Result<Option<StoredEntry>, UseCaseError> {
            Ok(None)
        }

        fn set(
            &self,
            _bucket: &BucketId,
            _key: &Key,
            _value: StoredValue,
            _expires_at: Option<OffsetDateTime>,
        ) -> Result<(), UseCaseError> {
            Ok(())
        }

        fn delete(&self, _bucket: &BucketId, _key: &Key) -> Result<bool, UseCaseError> {
            Ok(false)
        }

        fn list(
            &self,
            _bucket: &BucketId,
            _opts: &ListOpts<'_>,
        ) -> Result<Vec<ListEntry>, UseCaseError> {
            Ok(vec![])
        }

        fn increment(
            &self,
            _bucket: &BucketId,
            _key: &Key,
            _op: CounterOp,
            _expires_at: Option<OffsetDateTime>,
        ) -> Result<StoredValue, UseCaseError> {
            Err(UseCaseError::NotImplemented)
        }

        fn apply_batch(&self, _bucket: &BucketId, _ops: &[TxnOp]) -> Result<(), UseCaseError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn readiness_returns_503_when_storage_ping_fails() {
        let state = AppState::new(
            "0.1.0-test",
            Arc::new(SystemClock),
            Arc::new(AlwaysFailingBuckets) as Arc<dyn BucketRepository>,
            Arc::new(NoopKeys) as Arc<dyn KeyRepository>,
            Arc::new(SigningKey::new(Box::new(
                b"test-root-key-32-bytes-minimum!!".to_vec(),
            ))),
        );

        let resp = readiness_check(State(state)).await;
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);

        let body = resp.into_body();
        let bytes = BodyExt::collect(body).await.expect("body").to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&bytes).expect("json body");

        assert_eq!(
            json,
            serde_json::json!({
                "status": "degraded",
                "storage": "error",
                "version": "0.1.0-test"
            })
        );
    }
}
