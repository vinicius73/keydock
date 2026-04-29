use axum::Form;
use axum::extract::State;
use axum::response::Response;
use keydock_domain::TemporaryTokenClaims;
use keydock_state::AppState;
use keydock_usecase::mint;
use serde::{Deserialize, Serialize};
use time::Duration;
use tracing::{info, instrument};
use utoipa::ToSchema;

use crate::blocking;
use crate::error::{bad_request, not_found, service_unavailable};
use crate::extract::BucketAuth;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateTokenForm {
    /// Required non-empty prefix scope for the minted token.
    pub prefix: String,
    pub permissions: String,
    /// TTL seconds from `now`. Must be strictly positive; `<= 0` is rejected.
    pub ttl: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AccessTokenResponse {
    pub access_token: String,
}

fn parse_permissions(raw: &str) -> Result<keydock_domain::Permission, Response> {
    let mut p = keydock_domain::Permission::NONE;
    for part in raw.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        match part {
            "read" => p.read = true,
            "write" => p.write = true,
            "enumerate" => p.enumerate = true,
            "delete" => p.delete = true,
            _ => return Err(bad_request()),
        }
    }
    Ok(p)
}

#[utoipa::path(
    post,
    path = "/{bucket}/tokens/",
    params(
        ("bucket" = String, Path, description = "Bucket id"),
    ),
    request_body(
        content(
            (CreateTokenForm = "application/x-www-form-urlencoded"),
        ),
    ),
    responses(
        (status = 200, description = "Issued JWT", body = AccessTokenResponse),
        (status = 400, description = "Bad request", body = crate::error::ErrorBody),
        (status = 403, description = "Forbidden", body = crate::error::ErrorBody),
        (status = 404, description = "Bucket not found", body = crate::error::ErrorBody),
        (status = 503, description = "Signing unavailable", body = crate::error::ErrorBody),
        (status = 500, description = "Internal error", body = crate::error::ErrorBody),
    ),
    tag = "tokens"
)]
#[instrument(skip_all, name = "tokens::create_token")]
pub async fn create_token(
    State(state): State<AppState>,
    auth: BucketAuth,
    Form(form): Form<CreateTokenForm>,
) -> Result<axum::Json<AccessTokenResponse>, Response> {
    auth.require_admin()?;

    let buckets = state.buckets().clone();
    let bucket_id = auth.bucket_id.clone();
    let policy = blocking::spawn_usecase(move || buckets.get_policy(&bucket_id))
        .await?
        .ok_or_else(not_found)?;

    let signing_key = policy
        .signing_key
        .as_ref()
        .ok_or_else(service_unavailable)?;

    // `ttl` is seconds-from-now and must be strictly positive.
    // Negative or zero values would mint a token whose `exp <= iat`, which
    // `tokens::verify` later rejects as expired, so clients would receive a
    // `200 OK` for a token guaranteed to fail on first use.
    if form.ttl <= 0 {
        return Err(bad_request());
    }
    if form.prefix.is_empty() {
        return Err(bad_request());
    }

    let now = state.clock().now_utc();
    let exp = now
        .checked_add(Duration::seconds(form.ttl))
        .ok_or_else(bad_request)?;

    let permissions = parse_permissions(&form.permissions)?;
    let allowed_prefix = form.prefix.into_bytes();

    let claims = TemporaryTokenClaims {
        version: 1,
        bucket: auth.bucket_id.clone(),
        bucket_generation: policy.signing_key_generation,
        allowed_prefix,
        permissions,
        iat: now,
        exp,
    };

    let access_token = mint(&claims, signing_key).map_err(|_| service_unavailable())?;

    info!(
        bucket = %auth.bucket_id.as_str(),
        ttl_secs = form.ttl,
        prefix_len = claims.allowed_prefix.len(),
        permission_read = claims.permissions.read,
        permission_write = claims.permissions.write,
        permission_delete = claims.permissions.delete,
        permission_enumerate = claims.permissions.enumerate,
        signing_key_generation = policy.signing_key_generation,
        "token minted"
    );

    Ok(axum::Json(AccessTokenResponse { access_token }))
}
