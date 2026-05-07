use axum::response::Response;
use keydock_usecase::UseCaseError;
use tracing::instrument;

use crate::error::{internal_error, map_use_case_repo_err};

#[instrument(skip_all, name = "blocking::spawn_usecase")]
pub(crate) async fn spawn_usecase<T, F>(f: F) -> Result<T, Response>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, UseCaseError> + Send + 'static,
{
    let res = tokio::task::spawn_blocking(f).await.map_err(|e| {
        tracing::error!(error = %e, "spawn_blocking join failed");
        internal_error()
    })?;
    res.map_err(map_use_case_repo_err)
}
