use axum::Form;
use axum::body::Bytes;
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
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::json;
use tracing::{debug, instrument};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::error::{bad_request, internal_error, map_use_case_repo_err, not_acceptable, not_found};
use crate::extract::BucketAuth;

/// Hosted default TTL applied to buckets created without an explicit
/// `default_ttl`: 7 days (604800 seconds).
/// Clients that want "never expires by default" must send `default_ttl=0`
/// explicitly — the same signal `resolve_ttl` already treats as "no expiry".
const DEFAULT_BUCKET_TTL_SECS: u64 = 604_800;

/// Public snapshot of anonymous capabilities — mirrors [`Permission`] shape
/// but lives in `keydock-http` so we can derive `ToSchema` without leaking
/// OpenAPI deps into the domain crate.
#[derive(Debug, Serialize, ToSchema)]
pub struct AnonymousAccessView {
    pub read: bool,
    pub write: bool,
    pub enumerate: bool,
    pub delete: bool,
}

impl From<Permission> for AnonymousAccessView {
    fn from(p: Permission) -> Self {
        Self {
            read: p.read,
            write: p.write,
            enumerate: p.enumerate,
            delete: p.delete,
        }
    }
}

/// Public projection of [`BucketPolicy`] returned by `GET /{bucket}`.
///
/// Intentionally excludes every sensitive field: API key hashes and the raw
/// `signing_key` never leave the server. Only presence flags and rotation
/// metadata are surfaced so clients can negotiate token lifecycle.
#[derive(Debug, Serialize, ToSchema)]
pub struct BucketPolicyPublic {
    /// `default_ttl` in seconds (if configured); `None` means no default expiry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_ttl: Option<u64>,
    pub has_secret_key: bool,
    pub has_read_key: bool,
    pub has_write_key: bool,
    pub has_signing_key: bool,
    /// Bumps on every signing key rotation; lets clients invalidate caches.
    pub signing_key_generation: u64,
    pub anonymous_access: AnonymousAccessView,
}

fn public_policy_view(policy: &BucketPolicy) -> BucketPolicyPublic {
    BucketPolicyPublic {
        default_ttl: policy.default_ttl_secs,
        has_secret_key: policy.secret_key_hash.is_some(),
        has_read_key: policy.read_key_hash.is_some(),
        has_write_key: policy.write_key_hash.is_some(),
        has_signing_key: policy.signing_key.is_some(),
        signing_key_generation: policy.signing_key_generation,
        anonymous_access: policy.anonymous_access.into(),
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateBucketForm {
    pub email: String,
    pub secret_key: Option<String>,
    pub read_key: Option<String>,
    pub write_key: Option<String>,
    pub signing_key: Option<String>,
    pub default_ttl: Option<u64>,
}

/// JSON body for `PATCH /{bucket}`.
///
/// Each field uses `Option<Option<T>>` so the handler can distinguish three
/// distinct client intents:
///
/// - field absent → `None` → leave as-is (no-op).
/// - field `null` → `Some(None)` → clear the corresponding value.
/// - field with a value → `Some(Some(v))` → set/replace.
///
/// Empty strings are rejected with `400 bad_request` to avoid silent no-ops
/// and the `secret_key` field MUST NOT be cleared — removing it would orphan
/// the bucket (only admins can re-attach credentials).
#[derive(Debug, Default, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdatePolicyJson {
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub secret_key: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub read_key: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub write_key: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub signing_key: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub default_ttl: Option<Option<u64>>,
}

/// Serde helper that wraps the field in an extra `Some(...)` layer so the
/// outer `Option` models "present vs absent" while the inner one models
/// "value vs null". Combined with `#[serde(default)]` this yields the
/// three-way distinction documented on [`UpdatePolicyJson`].
fn deserialize_double_option<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    T::deserialize(deserializer).map(Some)
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
    Permission::anonymous_from_keys(
        policy.secret_key_hash.is_some(),
        policy.read_key_hash.is_some(),
        policy.write_key_hash.is_some(),
    )
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

/// Escapes only newline and carriage return in the `text` listing format.
/// The backslash is emitted literally so
/// the wire output matches the upstream contract byte-for-byte; clients
/// that need round-trip fidelity should use `format=json` or `jsonl`.
fn escape_text_segment(s: &str) -> String {
    s.replace('\n', "\\n").replace('\r', "\\r")
}

/// Minimal structural e-mail validation.
///
/// Accepts any input with exactly one `@`, a non-empty local-part, and a
/// domain that contains at least one `.` not located at either edge. This
/// intentionally stays below RFC 5322 because the `email` field is only an
/// administrative label for the bucket, not a contact channel. Stricter
/// validation may be added later behind configuration.
fn is_minimally_valid_email(raw: &str) -> bool {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return false;
    }
    let Some((local, domain)) = trimmed.split_once('@') else {
        return false;
    };
    if local.is_empty() || domain.contains('@') {
        return false;
    }
    if !domain.contains('.') || domain.starts_with('.') || domain.ends_with('.') {
        return false;
    }
    true
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
    if !is_minimally_valid_email(&form.email) {
        return Err(bad_request());
    }

    let secret_key = none_if_empty(form.secret_key);
    let read_key = none_if_empty(form.read_key);
    let write_key = none_if_empty(form.write_key);
    let signing_key = none_if_empty(form.signing_key);

    let anonymous_access = Permission::anonymous_from_keys(
        secret_key.is_some(),
        read_key.is_some(),
        write_key.is_some(),
    );

    let rk = state.root_key().as_ref();
    let hash = |raw: &Option<String>| -> Result<Option<Vec<u8>>, Response> {
        raw.as_deref()
            .map(|s| hash_api_key_or_fail(rk, s))
            .transpose()
    };
    let secret_key_hash = hash(&secret_key)?;
    let read_key_hash = hash(&read_key)?;
    let write_key_hash = hash(&write_key)?;

    // When the client omits `default_ttl`, fall back to the hosted default.
    // An explicit `0` keeps the "no expiry" semantics because `resolve_ttl`
    // already treats `Some(0)` the same as `None`.
    let default_ttl_secs = Some(form.default_ttl.unwrap_or(DEFAULT_BUCKET_TTL_SECS));

    let policy = BucketPolicy {
        default_ttl_secs,
        anonymous_access,
        secret_key_hash,
        read_key_hash,
        write_key_hash,
        signing_key: signing_key.map(|s| SigningKey::new(Box::new(s.into_bytes()))),
        signing_key_generation: 0,
    };

    let id = BucketId::new(Uuid::new_v4().to_string())
        .expect("Uuid::new_v4().to_string() is never empty; BucketId::new rejects only empty");
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
                    debug!(
                        bucket = %auth.bucket_id.as_str(),
                        "bucket listing empty (token prefix incompatible with query prefix)"
                    );
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

    let entry_count = entries.len();
    let body = render_list_body(fmt, &entries, include_values)?;
    let ct = list_content_type(fmt);
    let hv = HeaderValue::from_str(ct).map_err(|_| internal_error())?;
    debug!(
        bucket = %auth.bucket_id.as_str(),
        entry_count,
        include_values,
        "bucket listing completed"
    );
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
            (UpdatePolicyJson = "application/json"),
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
    body: Bytes,
) -> Result<StatusCode, Response> {
    auth.require_admin()?;

    // Parse manually so deserialization errors (malformed JSON, unknown
    // fields, wrong types) surface through our standard 400 envelope
    // instead of axum's default 422 plain-text response.
    let patch: UpdatePolicyJson = if body.is_empty() {
        UpdatePolicyJson::default()
    } else {
        serde_json::from_slice(&body).map_err(|_| bad_request())?
    };

    let mut policy = state
        .buckets()
        .get_policy(&auth.bucket_id)
        .map_err(map_use_case_repo_err)?
        .ok_or_else(not_found)?;

    apply_policy_patch(&mut policy, patch, state.root_key().as_ref())?;

    policy.anonymous_access = recompute_anonymous_access(&policy);

    state
        .buckets()
        .create_bucket(&auth.bucket_id, policy)
        .map_err(map_use_case_repo_err)?;

    Ok(StatusCode::NO_CONTENT)
}

/// Mutates `policy` according to the JSON patch.
///
/// Invariants:
/// - `secret_key` can be rotated but never cleared (the bucket would lose its
///   admin credential path). `null` on that field returns 400.
/// - Empty strings are rejected on every field: silent no-ops hide bugs.
/// - Rotations to `signing_key` bump `signing_key_generation` only when the
///   material actually changes, so replaying the same value is idempotent.
fn apply_policy_patch(
    policy: &mut BucketPolicy,
    patch: UpdatePolicyJson,
    root_key: &SigningKey,
) -> Result<(), Response> {
    if let Some(inner) = patch.secret_key {
        let s = inner.ok_or_else(bad_request)?;
        if s.is_empty() {
            return Err(bad_request());
        }
        policy.secret_key_hash = Some(hash_api_key_or_fail(root_key, &s)?);
    }
    if let Some(inner) = patch.read_key {
        policy.read_key_hash = apply_nullable_hash(inner, root_key)?;
    }
    if let Some(inner) = patch.write_key {
        policy.write_key_hash = apply_nullable_hash(inner, root_key)?;
    }
    if let Some(inner) = patch.signing_key {
        let new_bytes = match inner {
            None => None,
            Some(s) if s.is_empty() => return Err(bad_request()),
            Some(s) => Some(s.into_bytes()),
        };
        let current_bytes: Option<&[u8]> = policy
            .signing_key
            .as_ref()
            .map(|k: &SigningKey| k.expose_secret().as_slice());
        let changed = current_bytes != new_bytes.as_deref();
        if changed {
            policy.signing_key_generation += 1;
        }
        policy.signing_key = new_bytes.map(|b| SigningKey::new(Box::new(b)));
    }
    if let Some(inner) = patch.default_ttl {
        policy.default_ttl_secs = inner;
    }
    Ok(())
}

fn apply_nullable_hash(
    value: Option<String>,
    root_key: &SigningKey,
) -> Result<Option<Vec<u8>>, Response> {
    match value {
        None => Ok(None),
        Some(s) if s.is_empty() => Err(bad_request()),
        Some(s) => Ok(Some(hash_api_key_or_fail(root_key, &s)?)),
    }
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

#[utoipa::path(
    get,
    path = "/{bucket}",
    params(
        ("bucket" = String, Path, description = "Bucket id"),
    ),
    responses(
        (status = 200, description = "Bucket policy (public projection)", body = BucketPolicyPublic),
        (status = 401, description = "Unauthorized", body = crate::error::ErrorBody),
        (status = 403, description = "Forbidden", body = crate::error::ErrorBody),
        (status = 404, description = "Bucket not found", body = crate::error::ErrorBody),
        (status = 500, description = "Internal error", body = crate::error::ErrorBody),
    ),
    tag = "buckets"
)]
#[instrument(skip_all, name = "buckets::get_bucket_policy")]
pub async fn get_bucket_policy(
    State(state): State<AppState>,
    auth: BucketAuth,
) -> Result<axum::Json<BucketPolicyPublic>, Response> {
    // Only admins see policy metadata: enumerating keys is the read-side
    // capability, but the policy itself describes auth material
    // and lifetime defaults, so it stays scoped to `secret_key` holders.
    auth.require_admin()?;

    let policy = state
        .buckets()
        .get_policy(&auth.bucket_id)
        .map_err(map_use_case_repo_err)?
        .ok_or_else(not_found)?;

    Ok(axum::Json(public_policy_view(&policy)))
}

#[utoipa::path(
    head,
    path = "/{bucket}",
    params(
        ("bucket" = String, Path, description = "Bucket id"),
    ),
    responses(
        (status = 200, description = "Bucket exists; body is empty"),
        (status = 401, description = "Unauthorized", body = crate::error::ErrorBody),
        (status = 403, description = "Forbidden", body = crate::error::ErrorBody),
        (status = 404, description = "Bucket not found", body = crate::error::ErrorBody),
        (status = 500, description = "Internal error", body = crate::error::ErrorBody),
    ),
    tag = "buckets"
)]
#[instrument(skip_all, name = "buckets::head_bucket")]
pub async fn head_bucket(auth: BucketAuth) -> Result<StatusCode, Response> {
    // Mirrors `GET /{bucket}`'s admin gate: a successful HEAD confirms both
    // existence *and* admin access, so it must not be a probe for non-admins.
    // `BucketAuth` already failed with 404 if the bucket is unknown.
    auth.require_admin()?;
    Ok(StatusCode::OK)
}

/// OpenAPI-only stub for the trailing-slash alias `DELETE /{bucket}/`.
/// Shares the `delete_bucket` handler; documented separately so the public contract exposes both forms.
#[utoipa::path(
    delete,
    path = "/{bucket}/",
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
#[allow(dead_code)]
pub fn delete_bucket_slash_openapi() {}
