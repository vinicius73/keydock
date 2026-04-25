use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use keydock_domain::value::ValueKind;
use keydock_domain::{CounterOp, StoredValue};
use keydock_state::AppState;
use keydock_usecase::KeyService;
use serde::Deserialize;
use tracing::{debug, instrument};
use utoipa::{IntoParams, ToSchema};

use crate::blocking;
use crate::error::bad_request;
use crate::extract::{BucketAuth, parse_percent_encoded_key};

fn content_type_for_kind(kind: ValueKind) -> &'static str {
    match kind {
        ValueKind::Json => "application/json",
        ValueKind::Int64 | ValueKind::Float64 | ValueKind::Utf8 => "text/plain; charset=utf-8",
        ValueKind::Raw => "application/octet-stream",
    }
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

/// Query parameters for key write operations (`PUT`/`POST`/`PATCH /api/v1/{bucket}/{key}`).
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
#[into_params(parameter_in = Query)]
pub struct TtlQuery {
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
    let key_dom = parse_percent_encoded_key(&key)?;
    auth.require_read_on(&key_dom)?;
    let keys = state.keys().clone();
    let clock = state.clock().clone();
    let bucket_id = auth.bucket_id.clone();
    let key_dom2 = key_dom.clone();
    let entry = blocking::spawn_usecase(move || {
        KeyService::get(keys.as_ref(), clock.as_ref(), &bucket_id, &key_dom2)
    })
    .await?;
    debug!(
        bucket = %auth.bucket_id.as_str(),
        key_len = key_dom.as_bytes().len(),
        value_kind = ?entry.value.kind,
        "key read"
    );
    stored_value_response(&entry.value)
}

#[instrument(skip_all, name = "keys::head_key")]
#[utoipa::path(
    head,
    path = "/{bucket}/{key}",
    params(
        ("bucket" = String, Path, description = "Bucket id"),
        ("key" = String, Path, description = "Key (percent-encoded in the path)"),
    ),
    responses(
        (status = 200, description = "Key exists and has not expired; body is empty, Content-Type matches GET"),
        (status = 400, description = "Bad request", body = crate::error::ErrorBody),
        (status = 401, description = "Unauthorized", body = crate::error::ErrorBody),
        (status = 403, description = "Forbidden", body = crate::error::ErrorBody),
        (status = 404, description = "Key not found", body = crate::error::ErrorBody),
        (status = 500, description = "Internal error", body = crate::error::ErrorBody),
    ),
    tag = "keys"
)]
pub async fn head_key(
    State(state): State<AppState>,
    auth: BucketAuth,
    Path((_bucket, key)): Path<(String, String)>,
) -> Result<Response, Response> {
    // Same auth and TTL semantics as GET so existing tests translate cleanly;
    // the only difference is that we drop the body on the way out.
    let key_dom = parse_percent_encoded_key(&key)?;
    auth.require_read_on(&key_dom)?;
    let keys = state.keys().clone();
    let clock = state.clock().clone();
    let bucket_id = auth.bucket_id.clone();
    let key_dom2 = key_dom.clone();
    let entry = blocking::spawn_usecase(move || {
        KeyService::get(keys.as_ref(), clock.as_ref(), &bucket_id, &key_dom2)
    })
    .await?;

    let ct = content_type_for_kind(entry.value.kind);
    let hv = HeaderValue::from_static(ct);
    Ok((StatusCode::OK, [(header::CONTENT_TYPE, hv)], Bytes::new()).into_response())
}

/// OpenAPI-only stub: `PUT`/`POST` share [`put_key`] (raw body is not described via `utoipa` on the handler).
#[utoipa::path(
    put,
    post,
    path = "/{bucket}/{key}",
    params(
        ("bucket" = String, Path, description = "Bucket id"),
        ("key" = String, Path, description = "Key (percent-encoded in the path)"),
        TtlQuery,
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

#[instrument(skip_all, name = "keys::put_key")]
pub async fn put_key(
    State(state): State<AppState>,
    auth: BucketAuth,
    Path((_bucket, key)): Path<(String, String)>,
    Query(params): Query<TtlQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, Response> {
    let key_dom = parse_percent_encoded_key(&key)?;
    auth.require_write_on(&key_dom)?;
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .map(str::to_string);
    let keys = state.keys().clone();
    let clock = state.clock().clone();
    let bucket_id = auth.bucket_id.clone();
    let key_dom2 = key_dom.clone();
    let ttl = params.ttl;
    let default_ttl = auth.default_ttl_secs;
    let value = blocking::spawn_usecase(move || {
        KeyService::set(
            keys.as_ref(),
            clock.as_ref(),
            &bucket_id,
            &key_dom2,
            body,
            content_type.as_deref(),
            ttl,
            default_ttl,
        )
    })
    .await?;
    debug!(
        bucket = %auth.bucket_id.as_str(),
        key_len = key_dom.as_bytes().len(),
        value_kind = ?value.kind,
        value_bytes = value.payload.len(),
        ttl_query = ?params.ttl,
        "key stored"
    );
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
    let key_dom = parse_percent_encoded_key(&key)?;
    auth.require_delete_on(&key_dom)?;
    let keys = state.keys().clone();
    let bucket_id = auth.bucket_id.clone();
    let key_dom2 = key_dom.clone();
    blocking::spawn_usecase(move || KeyService::delete(keys.as_ref(), &bucket_id, &key_dom2))
        .await?;
    debug!(
        bucket = %auth.bucket_id.as_str(),
        key_len = key_dom.as_bytes().len(),
        "key deleted"
    );
    Ok(StatusCode::NO_CONTENT.into_response())
}

#[utoipa::path(
    patch,
    path = "/{bucket}/{key}",
    params(
        ("bucket" = String, Path, description = "Bucket id"),
        ("key" = String, Path, description = "Key (percent-encoded in the path)"),
        TtlQuery,
    ),
    request_body(content = String, description = "Counter delta: +N or -N (integer or float)"),
    responses(
        (status = 200, description = "New counter value (Content-Type depends on stored kind)"),
        (status = 400, description = "Bad request", body = crate::error::ErrorBody),
        (status = 401, description = "Unauthorized", body = crate::error::ErrorBody),
        (status = 403, description = "Forbidden", body = crate::error::ErrorBody),
        (status = 500, description = "Internal error", body = crate::error::ErrorBody),
    ),
    tag = "keys"
)]
#[instrument(skip_all, name = "keys::patch_key")]
pub async fn patch_key(
    State(state): State<AppState>,
    auth: BucketAuth,
    Path((_bucket, key)): Path<(String, String)>,
    Query(params): Query<TtlQuery>,
    body: Bytes,
) -> Result<Response, Response> {
    let key_dom = parse_percent_encoded_key(&key)?;
    auth.require_write_on(&key_dom)?;
    let op = CounterOp::parse(body.as_ref()).map_err(|_| bad_request())?;
    let keys = state.keys().clone();
    let clock = state.clock().clone();
    let bucket_id = auth.bucket_id.clone();
    let key_dom2 = key_dom.clone();
    let ttl = params.ttl;
    let default_ttl = auth.default_ttl_secs;
    let value = blocking::spawn_usecase(move || {
        KeyService::increment(
            keys.as_ref(),
            clock.as_ref(),
            &bucket_id,
            &key_dom2,
            op,
            ttl,
            default_ttl,
        )
    })
    .await?;
    debug!(
        bucket = %auth.bucket_id.as_str(),
        key_len = key_dom.as_bytes().len(),
        value_kind = ?value.kind,
        ttl_query = ?params.ttl,
        "counter updated"
    );
    stored_value_response(&value)
}
