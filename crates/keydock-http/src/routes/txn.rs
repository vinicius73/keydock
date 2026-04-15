//! Multi-key transaction endpoint (`POST /{bucket}`).

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use keydock_state::AppState;
use keydock_usecase::{KeyService, TxnOp, TxnService};
use serde::Deserialize;
use tracing::instrument;
use utoipa::ToSchema;

use crate::error::{bad_request, map_use_case_repo_err};
use crate::extract::{BucketAuth, parse_percent_encoded_key};

/// JSON command discriminator for a transaction step.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum TxnCmd {
    Set,
    Delete,
}

/// One operation inside `TxnRequest.txn`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct TxnItem {
    pub cmd: TxnCmd,
    pub key: String,
    pub value: Option<String>,
    #[serde(rename = "content_type")]
    pub content_type: Option<String>,
    pub ttl: Option<u64>,
}

/// Request body for `POST /{bucket}` (atomic batch).
#[derive(Debug, Deserialize, ToSchema)]
pub struct TxnRequest {
    pub txn: Vec<TxnItem>,
}

#[instrument(skip_all, name = "txn::execute_txn")]
#[utoipa::path(
    post,
    path = "/{bucket}",
    params(
        ("bucket" = String, Path, description = "Bucket id"),
    ),
    request_body(content = TxnRequest, description = "Atomic set/delete operations"),
    responses(
        (status = 204, description = "Transaction committed"),
        (status = 400, description = "Bad request", body = crate::error::ErrorBody),
        (status = 401, description = "Unauthorized", body = crate::error::ErrorBody),
        (status = 403, description = "Forbidden", body = crate::error::ErrorBody),
        (status = 500, description = "Internal error", body = crate::error::ErrorBody),
    ),
    tag = "transactions"
)]
pub async fn execute_txn(
    State(state): State<AppState>,
    auth: BucketAuth,
    Json(req): Json<TxnRequest>,
) -> Result<Response, Response> {
    let mut ops: Vec<TxnOp> = Vec::with_capacity(req.txn.len());
    for item in &req.txn {
        let key_dom = parse_percent_encoded_key(&item.key)?;
        match item.cmd {
            TxnCmd::Set => {
                auth.require_write_on(&key_dom)?;
                let raw = item.value.as_deref().ok_or_else(bad_request)?;
                let body = Bytes::copy_from_slice(raw.as_bytes());
                let value = KeyService::infer_stored_value(body, item.content_type.as_deref())
                    .map_err(map_use_case_repo_err)?;
                let expires_at = KeyService::resolve_ttl(
                    state.clock().as_ref(),
                    item.ttl,
                    auth.default_ttl_secs,
                )
                .map_err(map_use_case_repo_err)?;
                ops.push(TxnOp::Set {
                    key: key_dom,
                    value,
                    expires_at,
                });
            }
            TxnCmd::Delete => {
                auth.require_delete_on(&key_dom)?;
                ops.push(TxnOp::Delete { key: key_dom });
            }
        }
    }

    TxnService::execute(state.keys().as_ref(), &auth.bucket_id, &ops)
        .map_err(map_use_case_repo_err)?;

    Ok(StatusCode::NO_CONTENT.into_response())
}
