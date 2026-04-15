use axum::Form;
use axum::extract::State;
use axum::http::header;
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use keydock_domain::{BucketId, BucketPolicy, Permission, SigningKey};
use keydock_state::AppState;
use keydock_usecase::{UseCaseError, hash_credential};
use secrecy::ExposeSecret;
use serde::Deserialize;
use uuid::Uuid;

use crate::error::{bad_request, internal_error, not_found};
use crate::extract::BucketAuth;

#[derive(Debug, Deserialize)]
pub struct CreateBucketForm {
    pub email: String,
    pub secret_key: Option<String>,
    pub read_key: Option<String>,
    pub write_key: Option<String>,
    pub signing_key: Option<String>,
    pub default_ttl: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePolicyForm {
    pub secret_key: Option<String>,
    pub read_key: Option<String>,
    pub write_key: Option<String>,
    pub signing_key: Option<String>,
    pub default_ttl: Option<u64>,
}

fn none_if_empty(s: Option<String>) -> Option<String> {
    s.and_then(|v| if v.is_empty() { None } else { Some(v) })
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

    let policy = BucketPolicy {
        default_ttl_secs: form.default_ttl,
        anonymous_access,
        secret_key_hash: secret_key
            .as_ref()
            .map(|s| hash_credential(s, &state.root_key)),
        read_key_hash: read_key
            .as_ref()
            .map(|s| hash_credential(s, &state.root_key)),
        write_key_hash: write_key
            .as_ref()
            .map(|s| hash_credential(s, &state.root_key)),
        signing_key: signing_key.map(|s| SigningKey::new(Box::new(s.into_bytes()))),
        signing_key_generation: 0,
    };

    let id = BucketId::new(Uuid::new_v4().to_string()).map_err(|_| bad_request())?;
    state
        .buckets
        .create_bucket(&id, policy)
        .map_err(map_bucket_repo_err)?;

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

pub async fn list_bucket(
    State(_state): State<AppState>,
    auth: BucketAuth,
) -> Result<axum::Json<serde_json::Value>, Response> {
    auth.require_enumerate()?;
    Ok(axum::Json(serde_json::json!({ "keys": [] })))
}

pub async fn update_policy(
    State(state): State<AppState>,
    auth: BucketAuth,
    Form(form): Form<UpdatePolicyForm>,
) -> Result<StatusCode, Response> {
    auth.require_admin()?;

    let mut policy = state
        .buckets
        .get_policy(&auth.bucket_id)
        .map_err(map_bucket_repo_err)?
        .ok_or_else(not_found)?;

    if let Some(s) = none_if_empty(form.secret_key) {
        policy.secret_key_hash = Some(hash_credential(&s, &state.root_key));
    }
    if let Some(s) = none_if_empty(form.read_key) {
        policy.read_key_hash = Some(hash_credential(&s, &state.root_key));
    }
    if let Some(s) = none_if_empty(form.write_key) {
        policy.write_key_hash = Some(hash_credential(&s, &state.root_key));
    }
    if let Some(s) = none_if_empty(form.signing_key) {
        let bytes = s.into_bytes();
        let changed = policy
            .signing_key
            .as_ref()
            .map(|k| k.expose_secret().as_slice() != bytes.as_slice())
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
        .buckets
        .create_bucket(&auth.bucket_id, policy)
        .map_err(map_bucket_repo_err)?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn delete_bucket(
    State(state): State<AppState>,
    auth: BucketAuth,
) -> Result<StatusCode, Response> {
    auth.require_admin()?;

    state
        .buckets
        .delete_bucket(&auth.bucket_id)
        .map_err(map_bucket_repo_err)?;

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) fn map_bucket_repo_err(err: UseCaseError) -> Response {
    match err {
        UseCaseError::Storage(msg) => {
            tracing::error!(error = %msg, "bucket repository error");
            internal_error()
        }
        other => {
            tracing::error!(?other, "bucket repository error");
            internal_error()
        }
    }
}
