use axum::extract::Path;
use axum::extract::State;
use axum::response::Response;
use bytes::Bytes;
use keydock_domain::Key;
use keydock_state::AppState;

use crate::error::bad_request;
use crate::error::not_implemented;
use crate::extract::BucketAuth;

fn parse_key(key: &str) -> Result<Key, Response> {
    Key::from_bytes(Bytes::copy_from_slice(key.as_bytes())).map_err(|_| bad_request())
}

pub async fn get_key(
    State(_state): State<AppState>,
    auth: BucketAuth,
    Path((_bucket, key)): Path<(String, String)>,
) -> Result<axum::Json<serde_json::Value>, Response> {
    let key_dom = parse_key(&key)?;
    auth.require_read_on(&key_dom)?;
    Ok(axum::Json(serde_json::json!({ "ok": true })))
}

pub async fn put_key(
    State(_state): State<AppState>,
    auth: BucketAuth,
    Path((_bucket, key)): Path<(String, String)>,
) -> Result<axum::Json<serde_json::Value>, Response> {
    let key_dom = parse_key(&key)?;
    auth.require_write_on(&key_dom)?;
    Ok(axum::Json(serde_json::json!({ "ok": true })))
}

pub async fn delete_key(
    State(_state): State<AppState>,
    auth: BucketAuth,
    Path((_bucket, key)): Path<(String, String)>,
) -> Result<axum::Json<serde_json::Value>, Response> {
    let key_dom = parse_key(&key)?;
    auth.require_delete_on(&key_dom)?;
    Ok(axum::Json(serde_json::json!({ "ok": true })))
}

pub async fn patch_key(
    State(_state): State<AppState>,
    _auth: BucketAuth,
    Path((bucket, key)): Path<(String, String)>,
) -> Response {
    not_implemented(format!("PATCH /{bucket}/{key}"))
}
