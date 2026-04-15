use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use keydock_domain::Key;
use keydock_domain::StoredValue;
use keydock_domain::value::ValueKind;
use keydock_state::AppState;
use keydock_usecase::KeyService;
use percent_encoding::percent_decode_str;
use serde::Deserialize;
use tracing::instrument;
use utoipa::{IntoParams, ToSchema};

use crate::error::{bad_request, map_use_case_repo_err, not_implemented};
use crate::extract::BucketAuth;

fn parse_key(key: &str) -> Result<Key, Response> {
    let decoded: Vec<u8> = percent_decode_str(key).collect();
    Key::from_bytes(Bytes::from(decoded)).map_err(|_| bad_request())
}

fn content_type_for_kind(kind: ValueKind) -> &'static str {
    match kind {
        ValueKind::Json => "application/json",
        ValueKind::Int64 | ValueKind::Float64 | ValueKind::Utf8 => "text/plain; charset=utf-8",
        ValueKind::Raw => "application/octet-stream",
    }
}

fn parse_content_type_header(headers: &HeaderMap) -> Option<&str> {
    headers.get(header::CONTENT_TYPE)?.to_str().ok()
}

fn stored_value_response(value: &StoredValue) -> Result<Response, Response> {
    let ct = content_type_for_kind(value.kind);
    let hv = HeaderValue::from_static(ct);
    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, hv)],
        value.payload.clone(),
    )
        .into_response())
}

/// Query parameters for `PUT`/`POST /{bucket}/{key}`.
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
#[into_params(parameter_in = Query)]
pub struct PutKeyParams {
    /// TTL in seconds (overrides bucket default when set).
    pub ttl: Option<u64>,
}

#[instrument(skip_all, name = "keys::get_key")]
#[utoipa::path(
    get,
    path = "/{bucket}/{key}",
    params(
        ("bucket" = String, Path, description = "Bucket id"),
        ("key" = String, Path, description = "Key (percent-encoded in the path)"),
    ),
    responses(
        (status = 200, description = "Stored value (Content-Type depends on stored kind)"),
        (status = 400, description = "Bad request", body = crate::error::ErrorBody),
        (status = 401, description = "Unauthorized", body = crate::error::ErrorBody),
        (status = 403, description = "Forbidden", body = crate::error::ErrorBody),
        (status = 404, description = "Key not found", body = crate::error::ErrorBody),
        (status = 500, description = "Internal error", body = crate::error::ErrorBody),
    ),
    tag = "keys"
)]
pub async fn get_key(
    State(state): State<AppState>,
    auth: BucketAuth,
    Path((_bucket, key)): Path<(String, String)>,
) -> Result<Response, Response> {
    let key_dom = parse_key(&key)?;
    auth.require_read_on(&key_dom)?;
    let entry = KeyService::get(state.keys().as_ref(), &auth.bucket_id, &key_dom)
        .map_err(map_use_case_repo_err)?;
    stored_value_response(&entry.value)
}

/// OpenAPI-only stub: `PUT`/`POST` share [`put_key`] (raw body is not described via `utoipa` on the handler).
#[utoipa::path(
    put,
    post,
    path = "/{bucket}/{key}",
    params(
        ("bucket" = String, Path, description = "Bucket id"),
        ("key" = String, Path, description = "Key (percent-encoded in the path)"),
        PutKeyParams,
    ),
    responses(
        (status = 200, description = "Stored value echoed (Content-Type depends on inferred kind)"),
        (status = 400, description = "Bad request", body = crate::error::ErrorBody),
        (status = 401, description = "Unauthorized", body = crate::error::ErrorBody),
        (status = 403, description = "Forbidden", body = crate::error::ErrorBody),
        (status = 500, description = "Internal error", body = crate::error::ErrorBody),
    ),
    tag = "keys"
)]
#[allow(dead_code)]
pub fn put_key_openapi() {}

/// OpenAPI-only stub: [`patch_key`] currently returns 501 until implemented.
#[utoipa::path(
    patch,
    path = "/{bucket}/{key}",
    params(
        ("bucket" = String, Path, description = "Bucket id"),
        ("key" = String, Path, description = "Key (percent-encoded in the path)"),
    ),
    responses(
        (status = 501, description = "Not implemented", body = crate::error::ErrorBody),
    ),
    tag = "keys"
)]
#[allow(dead_code)]
pub fn patch_key_openapi() {}

#[instrument(skip_all, name = "keys::put_key")]
pub async fn put_key(
    State(state): State<AppState>,
    auth: BucketAuth,
    Path((_bucket, key)): Path<(String, String)>,
    Query(params): Query<PutKeyParams>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, Response> {
    let key_dom = parse_key(&key)?;
    auth.require_write_on(&key_dom)?;
    let content_type = parse_content_type_header(&headers);
    let value = KeyService::set(
        state.keys().as_ref(),
        state.clock().as_ref(),
        &auth.bucket_id,
        &key_dom,
        body,
        content_type,
        params.ttl,
        auth.default_ttl_secs,
    )
    .map_err(map_use_case_repo_err)?;
    stored_value_response(&value)
}

#[instrument(skip_all, name = "keys::delete_key")]
#[utoipa::path(
    delete,
    path = "/{bucket}/{key}",
    params(
        ("bucket" = String, Path, description = "Bucket id"),
        ("key" = String, Path, description = "Key (percent-encoded in the path)"),
    ),
    responses(
        (status = 204, description = "Deleted"),
        (status = 400, description = "Bad request", body = crate::error::ErrorBody),
        (status = 401, description = "Unauthorized", body = crate::error::ErrorBody),
        (status = 403, description = "Forbidden", body = crate::error::ErrorBody),
        (status = 404, description = "Key not found", body = crate::error::ErrorBody),
        (status = 500, description = "Internal error", body = crate::error::ErrorBody),
    ),
    tag = "keys"
)]
pub async fn delete_key(
    State(state): State<AppState>,
    auth: BucketAuth,
    Path((_bucket, key)): Path<(String, String)>,
) -> Result<Response, Response> {
    let key_dom = parse_key(&key)?;
    auth.require_delete_on(&key_dom)?;
    KeyService::delete(state.keys().as_ref(), &auth.bucket_id, &key_dom)
        .map_err(map_use_case_repo_err)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

#[instrument(skip_all, name = "keys::patch_key")]
pub async fn patch_key(
    State(_state): State<AppState>,
    _auth: BucketAuth,
    Path((bucket, key)): Path<(String, String)>,
) -> Response {
    not_implemented(format!("PATCH /{bucket}/{key}"))
}
