use axum::Form;
use axum::extract::{Query, State};
use axum::http::header;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use keydock_domain::value::ValueKind;
use keydock_domain::{BucketId, BucketPolicy, Permission, SigningKey};
use keydock_state::AppState;
use keydock_usecase::hash_credential;
use keydock_usecase::{KeyService, ListEntry, ListOptsInput, ResolvedIdentity};
use secrecy::ExposeSecret;
use serde::Deserialize;
use serde_json::json;
use tracing::instrument;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::error::{bad_request, internal_error, map_use_case_repo_err, not_acceptable, not_found};
use crate::extract::BucketAuth;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateBucketForm {
    pub email: String,
    pub secret_key: Option<String>,
    pub read_key: Option<String>,
    pub write_key: Option<String>,
    pub signing_key: Option<String>,
    pub default_ttl: Option<u64>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdatePolicyForm {
    pub secret_key: Option<String>,
    pub read_key: Option<String>,
    pub write_key: Option<String>,
    pub signing_key: Option<String>,
    pub default_ttl: Option<u64>,
}

/// Query parameters for `GET /{bucket}/` (listing).
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
#[into_params(parameter_in = Query)]
pub struct ListBucketParams {
    /// Restrict listing to keys starting with this byte prefix (UTF-8 string from query).
    pub prefix: Option<String>,
    /// Maximum number of keys to return (default 10000).
    pub limit: Option<usize>,
    /// Number of keys to skip after ordering and expiry filter (default 0).
    pub skip: Option<usize>,
    /// When `true`, iterate in reverse lexicographic order.
    pub reverse: Option<bool>,
    /// When `true`, include values in the response body.
    pub values: Option<bool>,
    /// Response format: `text`, `json`, or `jsonl` (overrides `Accept` when set).
    pub format: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ListFormat {
    Text,
    Json,
    Jsonl,
}

fn none_if_empty(s: Option<String>) -> Option<String> {
    s.and_then(|v| if v.is_empty() { None } else { Some(v) })
}

fn hash_api_key_or_fail(root_key: &SigningKey, raw: &str) -> Result<Vec<u8>, Response> {
    hash_credential(raw, root_key).map_err(|e| {
        tracing::error!(error = %e, "failed to hash API key material");
        internal_error()
    })
}

fn recompute_anonymous_access(policy: &BucketPolicy) -> Permission {
    let has_s = policy.secret_key_hash.is_some();
    let has_r = policy.read_key_hash.is_some();
    let has_w = policy.write_key_hash.is_some();
    Permission {
        read: !has_r,
        write: !has_w,
        enumerate: !has_r,
        delete: !has_s && !has_r && !has_w,
    }
}

/// Intersects token scope prefix with optional `?prefix=` (incompatible combinations return `None`).
fn combine_scoped_prefix(scope: &[u8], requested: Option<&[u8]>) -> Option<Vec<u8>> {
    match requested {
        None => Some(scope.to_vec()),
        Some(req) => {
            if req.starts_with(scope) {
                Some(req.to_vec())
            } else if scope.starts_with(req) {
                Some(scope.to_vec())
            } else {
                None
            }
        }
    }
}

fn resolve_list_format(
    params: &ListBucketParams,
    headers: &HeaderMap,
) -> Result<ListFormat, Response> {
    if let Some(ref f) = params.format {
        return match f.to_ascii_lowercase().as_str() {
            "text" => Ok(ListFormat::Text),
            "json" => Ok(ListFormat::Json),
            "jsonl" => Ok(ListFormat::Jsonl),
            _ => Err(not_acceptable()),
        };
    }
    if let Some(accept) = headers.get(header::ACCEPT).and_then(|h| h.to_str().ok()) {
        if accept.contains("application/json") {
            return Ok(ListFormat::Json);
        }
        if accept.contains("application/x-ndjson") || accept.contains("application/ndjson") {
            return Ok(ListFormat::Jsonl);
        }
        if accept.contains("text/plain") {
            return Ok(ListFormat::Text);
        }
    }
    Ok(ListFormat::Text)
}

fn key_to_json_string(key: &keydock_domain::Key) -> String {
    String::from_utf8_lossy(key.as_bytes()).into_owned()
}

fn escape_text_segment(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn stored_value_to_json(v: &keydock_domain::StoredValue) -> Result<serde_json::Value, Response> {
    match v.kind {
        ValueKind::Json => serde_json::from_slice(v.payload.as_ref()).map_err(|_| internal_error()),
        ValueKind::Int64 => {
            let s = std::str::from_utf8(v.payload.as_ref()).map_err(|_| internal_error())?;
            let n: i64 = s.trim().parse().map_err(|_| internal_error())?;
            Ok(json!(n))
        }
        ValueKind::Float64 => {
            let s = std::str::from_utf8(v.payload.as_ref()).map_err(|_| internal_error())?;
            let n: f64 = s.trim().parse().map_err(|_| internal_error())?;
            Ok(serde_json::Number::from_f64(n)
                .map(serde_json::Value::Number)
                .ok_or_else(internal_error)?)
        }
        ValueKind::Utf8 => {
            let s = std::str::from_utf8(v.payload.as_ref()).map_err(|_| internal_error())?;
            Ok(json!(s))
        }
        ValueKind::Raw => Ok(serde_json::Value::Array(
            v.payload
                .iter()
                .copied()
                .map(serde_json::Value::from)
                .collect(),
        )),
    }
}

fn list_content_type(fmt: ListFormat) -> &'static str {
    match fmt {
        ListFormat::Text => "text/plain; charset=utf-8",
        ListFormat::Json => "application/json",
        ListFormat::Jsonl => "application/x-ndjson",
    }
}

fn render_list_body(
    fmt: ListFormat,
    entries: &[ListEntry],
    include_values: bool,
) -> Result<Vec<u8>, Response> {
    match fmt {
        ListFormat::Text => render_list_text(entries, include_values),
        ListFormat::Json => render_list_json(entries, include_values),
        ListFormat::Jsonl => render_list_jsonl(entries, include_values),
    }
}

fn render_list_text(entries: &[ListEntry], include_values: bool) -> Result<Vec<u8>, Response> {
    let mut out = String::new();
    for (i, row) in entries.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let key_str = key_to_json_string(&row.key);
        let esc_key = escape_text_segment(&key_str);
        if include_values {
            let val = row.value.as_ref().ok_or_else(internal_error)?;
            let val_str = String::from_utf8_lossy(val.payload.as_ref()).into_owned();
            let esc_val = escape_text_segment(&val_str);
            out.push_str(&esc_key);
            out.push('=');
            out.push_str(&esc_val);
        } else {
            out.push_str(&esc_key);
        }
    }
    Ok(out.into_bytes())
}

fn render_list_json(entries: &[ListEntry], include_values: bool) -> Result<Vec<u8>, Response> {
    let v = if include_values {
        let mut rows = Vec::with_capacity(entries.len());
        for row in entries {
            let k = key_to_json_string(&row.key);
            let val = row.value.as_ref().ok_or_else(internal_error)?;
            let jv = stored_value_to_json(val)?;
            rows.push(json!([k, jv]));
        }
        serde_json::Value::Array(rows)
    } else {
        let keys: Vec<String> = entries.iter().map(|r| key_to_json_string(&r.key)).collect();
        json!(keys)
    };
    serde_json::to_vec(&v).map_err(|_| internal_error())
}

fn render_list_jsonl(entries: &[ListEntry], include_values: bool) -> Result<Vec<u8>, Response> {
    let mut buf: Vec<u8> = Vec::new();
    for row in entries {
        let line = if include_values {
            let k = key_to_json_string(&row.key);
            let val = row.value.as_ref().ok_or_else(internal_error)?;
            let jv = stored_value_to_json(val)?;
            json!([k, jv])
        } else {
            json!(key_to_json_string(&row.key))
        };
        let mut chunk = serde_json::to_vec(&line).map_err(|_| internal_error())?;
        chunk.push(b'\n');
        buf.extend_from_slice(&chunk);
    }
    Ok(buf)
}

#[utoipa::path(
    post,
    path = "/",
    request_body(
        content(
            (CreateBucketForm = "application/x-www-form-urlencoded"),
        ),
    ),
    responses(
        (status = 200, description = "New bucket id as UTF-8 text (text/plain)"),
        (status = 400, description = "Bad request", body = crate::error::ErrorBody),
        (status = 500, description = "Internal error", body = crate::error::ErrorBody),
    ),
    tag = "buckets"
)]
#[instrument(skip_all, name = "buckets::create_bucket")]
pub async fn create_bucket(
    State(state): State<AppState>,
    Form(form): Form<CreateBucketForm>,
) -> Result<impl IntoResponse, Response> {
    if form.email.trim().is_empty() {
        return Err(bad_request());
    }

    let secret_key = none_if_empty(form.secret_key);
    let read_key = none_if_empty(form.read_key);
    let write_key = none_if_empty(form.write_key);
    let signing_key = none_if_empty(form.signing_key);

    let has_s = secret_key.is_some();
    let has_r = read_key.is_some();
    let has_w = write_key.is_some();

    let anonymous_access = Permission {
        read: !has_r,
        write: !has_w,
        enumerate: !has_r,
        delete: !has_s && !has_r && !has_w,
    };

    let rk = state.root_key().as_ref();
    let secret_key_hash = match secret_key.as_ref() {
        Some(s) => Some(hash_api_key_or_fail(rk, s)?),
        None => None,
    };
    let read_key_hash = match read_key.as_ref() {
        Some(s) => Some(hash_api_key_or_fail(rk, s)?),
        None => None,
    };
    let write_key_hash = match write_key.as_ref() {
        Some(s) => Some(hash_api_key_or_fail(rk, s)?),
        None => None,
    };

    let policy = BucketPolicy {
        default_ttl_secs: form.default_ttl,
        anonymous_access,
        secret_key_hash,
        read_key_hash,
        write_key_hash,
        signing_key: signing_key.map(|s| SigningKey::new(Box::new(s.into_bytes()))),
        signing_key_generation: 0,
    };

    let id = BucketId::new(Uuid::new_v4().to_string()).map_err(|_| bad_request())?;
    state
        .buckets()
        .create_bucket(&id, policy)
        .map_err(map_use_case_repo_err)?;

    let body = id.as_str().to_string();
    Ok((
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        )],
        body,
    ))
}

#[utoipa::path(
    get,
    path = "/{bucket}/",
    params(
        ("bucket" = String, Path, description = "Bucket id"),
        ListBucketParams,
    ),
    responses(
        (status = 200, description = "Key listing (format from ?format= or Accept: text/plain, application/json, application/x-ndjson)"),
        (status = 400, description = "Bad request", body = crate::error::ErrorBody),
        (status = 401, description = "Unauthorized", body = crate::error::ErrorBody),
        (status = 403, description = "Forbidden", body = crate::error::ErrorBody),
        (status = 404, description = "Bucket not found", body = crate::error::ErrorBody),
        (status = 406, description = "Unknown format", body = crate::error::ErrorBody),
        (status = 500, description = "Internal error", body = crate::error::ErrorBody),
    ),
    tag = "buckets"
)]
#[instrument(skip_all, name = "buckets::list_bucket")]
pub async fn list_bucket(
    State(state): State<AppState>,
    auth: BucketAuth,
    Query(params): Query<ListBucketParams>,
    headers: HeaderMap,
) -> Result<Response, Response> {
    auth.require_enumerate()?;
    let fmt = resolve_list_format(&params, &headers)?;
    let include_values = params.values.unwrap_or(false);

    let prefix_for_repo: Option<Vec<u8>> = match &auth.identity {
        ResolvedIdentity::Scoped { key_prefix, .. } if !key_prefix.is_empty() => {
            let req = params.prefix.as_deref().map(str::as_bytes);
            match combine_scoped_prefix(key_prefix, req) {
                Some(p) => Some(p),
                None => {
                    let body = render_list_body(fmt, &[], include_values)?;
                    let ct = list_content_type(fmt);
                    let hv = HeaderValue::from_str(ct).map_err(|_| internal_error())?;
                    return Ok((StatusCode::OK, [(header::CONTENT_TYPE, hv)], body).into_response());
                }
            }
        }
        _ => params.prefix.as_ref().map(|s| s.as_bytes().to_vec()),
    };

    let entries = KeyService::list(
        state.keys().as_ref(),
        state.clock().as_ref(),
        &auth.bucket_id,
        ListOptsInput {
            prefix: prefix_for_repo,
            limit: params.limit,
            skip: params.skip,
            reverse: params.reverse,
            include_values: Some(include_values),
        },
    )
    .map_err(map_use_case_repo_err)?;

    let body = render_list_body(fmt, &entries, include_values)?;
    let ct = list_content_type(fmt);
    let hv = HeaderValue::from_str(ct).map_err(|_| internal_error())?;
    Ok((StatusCode::OK, [(header::CONTENT_TYPE, hv)], body).into_response())
}

#[utoipa::path(
    patch,
    path = "/{bucket}",
    params(
        ("bucket" = String, Path, description = "Bucket id"),
    ),
    request_body(
        content(
            (UpdatePolicyForm = "application/x-www-form-urlencoded"),
        ),
    ),
    responses(
        (status = 204, description = "Policy updated"),
        (status = 400, description = "Bad request", body = crate::error::ErrorBody),
        (status = 403, description = "Forbidden", body = crate::error::ErrorBody),
        (status = 404, description = "Bucket not found", body = crate::error::ErrorBody),
        (status = 500, description = "Internal error", body = crate::error::ErrorBody),
    ),
    tag = "buckets"
)]
#[instrument(skip_all, name = "buckets::update_policy")]
pub async fn update_policy(
    State(state): State<AppState>,
    auth: BucketAuth,
    Form(form): Form<UpdatePolicyForm>,
) -> Result<StatusCode, Response> {
    auth.require_admin()?;

    let mut policy = state
        .buckets()
        .get_policy(&auth.bucket_id)
        .map_err(map_use_case_repo_err)?
        .ok_or_else(not_found)?;

    let rk = state.root_key().as_ref();
    if let Some(s) = none_if_empty(form.secret_key) {
        policy.secret_key_hash = Some(hash_api_key_or_fail(rk, &s)?);
    }
    if let Some(s) = none_if_empty(form.read_key) {
        policy.read_key_hash = Some(hash_api_key_or_fail(rk, &s)?);
    }
    if let Some(s) = none_if_empty(form.write_key) {
        policy.write_key_hash = Some(hash_api_key_or_fail(rk, &s)?);
    }
    if let Some(s) = none_if_empty(form.signing_key) {
        let bytes = s.into_bytes();
        let changed = policy
            .signing_key
            .as_ref()
            .map(|k: &SigningKey| k.expose_secret().as_slice() != bytes.as_slice())
            .unwrap_or(true);
        if changed {
            policy.signing_key_generation += 1;
        }
        policy.signing_key = Some(SigningKey::new(Box::new(bytes)));
    }
    if let Some(ttl) = form.default_ttl {
        policy.default_ttl_secs = Some(ttl);
    }

    policy.anonymous_access = recompute_anonymous_access(&policy);

    state
        .buckets()
        .create_bucket(&auth.bucket_id, policy)
        .map_err(map_use_case_repo_err)?;

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/{bucket}",
    params(
        ("bucket" = String, Path, description = "Bucket id"),
    ),
    responses(
        (status = 204, description = "Bucket deleted"),
        (status = 403, description = "Forbidden", body = crate::error::ErrorBody),
        (status = 404, description = "Bucket not found", body = crate::error::ErrorBody),
        (status = 500, description = "Internal error", body = crate::error::ErrorBody),
    ),
    tag = "buckets"
)]
#[instrument(skip_all, name = "buckets::delete_bucket")]
pub async fn delete_bucket(
    State(state): State<AppState>,
    auth: BucketAuth,
) -> Result<StatusCode, Response> {
    auth.require_admin()?;

    state
        .buckets()
        .delete_bucket(&auth.bucket_id)
        .map_err(map_use_case_repo_err)?;

    Ok(StatusCode::NO_CONTENT)
}

/// OpenAPI-only stub: `GET /{bucket}` is wired in the HTTP router and returns 501 until M5.
#[utoipa::path(
    get,
    path = "/{bucket}",
    params(
        ("bucket" = String, Path, description = "Bucket id"),
    ),
    responses(
        (status = 501, description = "Not implemented", body = crate::error::ErrorBody),
    ),
    tag = "buckets"
)]
#[allow(dead_code)]
pub fn get_bucket_reserved_openapi() {}
