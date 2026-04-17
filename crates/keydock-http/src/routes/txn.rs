//! Multi-key transaction endpoint (`POST /{bucket}`).

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use keydock_state::AppState;
use keydock_usecase::{KeyService, TxnOp, TxnService};
use serde::Deserialize;
use serde_json::Value as JsonValue;
use tracing::instrument;
use utoipa::ToSchema;

use crate::error::{bad_request, map_use_case_repo_err};
use crate::extract::{BucketAuth, parse_percent_encoded_key};

/// `set` variant: stores `value` at the given user key.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TxnSet {
    /// User key to write (percent-encoded like path keys).
    pub set: String,
    /// Value payload. Strings fall through to the default content-type inference;
    /// numbers, booleans, arrays and objects are stored as JSON (`application/json`).
    #[schema(value_type = Object)]
    pub value: JsonValue,
    /// Optional TTL in seconds. Zero or absent means "no override".
    #[serde(default)]
    pub ttl: Option<u64>,
}

/// `delete` variant: removes the given user key atomically.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TxnDelete {
    /// User key to remove.
    pub delete: String,
}

/// One operation inside `TxnRequest.txn`. Discriminated by the presence of `set`
/// or `delete`. Items carrying both, neither, or unknown fields are rejected as
/// `400 bad_request`.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum TxnItem {
    Set(TxnSet),
    Delete(TxnDelete),
}

/// Request body for `POST /{bucket}` (atomic batch).
#[derive(Debug, Deserialize, ToSchema)]
pub struct TxnRequest {
    pub txn: Vec<TxnItem>,
}

/// Marker content-type used when the JSON `value` is not a string: forces
/// `ValueKind::Json` in the inference layer so numbers/booleans are preserved
/// as JSON instead of being re-interpreted as Int64/Float64 scalars.
const JSON_CONTENT_TYPE: &str = "application/json";

/// Converts a JSON `value` from a `set` operation into the `(body, content_type)`
/// pair consumed by [`KeyService::infer_stored_value`]. Rejects `null`.
fn json_value_to_body(value: JsonValue) -> Result<(Bytes, Option<&'static str>), Response> {
    match value {
        JsonValue::Null => Err(bad_request()),
        JsonValue::String(s) => Ok((Bytes::from(s.into_bytes()), None)),
        other => {
            let bytes = serde_json::to_vec(&other).map_err(|_| bad_request())?;
            Ok((Bytes::from(bytes), Some(JSON_CONTENT_TYPE)))
        }
    }
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
    body: Bytes,
) -> Result<Response, Response> {
    // Parse manually so that malformed payloads and shape-mismatch errors both
    // return the canonical `400 bad_request` envelope instead of Axum's default
    // 400/422 plain-text responses.
    let req: TxnRequest = serde_json::from_slice(&body).map_err(|_| bad_request())?;
    let mut ops: Vec<TxnOp> = Vec::with_capacity(req.txn.len());
    for item in req.txn {
        match item {
            TxnItem::Set(TxnSet { set, value, ttl }) => {
                let key_dom = parse_percent_encoded_key(&set)?;
                auth.require_write_on(&key_dom)?;
                let (body, content_type) = json_value_to_body(value)?;
                let stored = KeyService::infer_stored_value(body, content_type)
                    .map_err(map_use_case_repo_err)?;
                let expires_at =
                    KeyService::resolve_ttl(state.clock().as_ref(), ttl, auth.default_ttl_secs)
                        .map_err(map_use_case_repo_err)?;
                ops.push(TxnOp::Set {
                    key: key_dom,
                    value: stored,
                    expires_at,
                });
            }
            TxnItem::Delete(TxnDelete { delete }) => {
                let key_dom = parse_percent_encoded_key(&delete)?;
                auth.require_delete_on(&key_dom)?;
                ops.push(TxnOp::Delete { key: key_dom });
            }
        }
    }

    TxnService::execute(state.keys().as_ref(), &auth.bucket_id, &ops)
        .map_err(map_use_case_repo_err)?;

    Ok(StatusCode::NO_CONTENT.into_response())
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rstest::rstest;
    use serde_json::json;

    use super::*;

    #[test]
    fn parses_set_variant_with_string_value() {
        let req: TxnRequest = serde_json::from_value(json!({
            "txn": [{ "set": "k", "value": "v" }]
        }))
        .expect("parse");
        assert_eq!(req.txn.len(), 1);
        match &req.txn[0] {
            TxnItem::Set(s) => {
                assert_eq!(s.set, "k");
                assert_eq!(s.value, JsonValue::String("v".to_string()));
                assert_eq!(s.ttl, None);
            }
            TxnItem::Delete(_) => panic!("expected Set"),
        }
    }

    #[test]
    fn parses_set_variant_with_json_object_value() {
        let req: TxnRequest = serde_json::from_value(json!({
            "txn": [{ "set": "k", "value": { "a": 1 }, "ttl": 60 }]
        }))
        .expect("parse");
        match &req.txn[0] {
            TxnItem::Set(s) => {
                assert_eq!(s.ttl, Some(60));
                assert_eq!(s.value, json!({ "a": 1 }));
            }
            TxnItem::Delete(_) => panic!("expected Set"),
        }
    }

    #[test]
    fn parses_delete_variant() {
        let req: TxnRequest = serde_json::from_value(json!({
            "txn": [{ "delete": "k" }]
        }))
        .expect("parse");
        match &req.txn[0] {
            TxnItem::Delete(d) => assert_eq!(d.delete, "k"),
            TxnItem::Set(_) => panic!("expected Delete"),
        }
    }

    #[rstest]
    #[case::legacy_cmd_shape(json!({"txn":[{"cmd":"set","key":"k","value":"v"}]}))]
    #[case::both_set_and_delete(json!({"txn":[{"set":"a","delete":"b","value":"v"}]}))]
    #[case::set_without_value(json!({"txn":[{"set":"k"}]}))]
    #[case::extra_unknown_field(json!({"txn":[{"set":"k","value":"v","extra":true}]}))]
    fn rejects_invalid_shapes(#[case] payload: serde_json::Value) {
        let res: Result<TxnRequest, _> = serde_json::from_value(payload);
        assert!(res.is_err(), "expected deserialization error");
    }

    #[test]
    fn json_value_to_body_string_keeps_bytes_and_no_content_type() {
        let (body, ct) = json_value_to_body(JsonValue::String("olá".into())).expect("ok");
        assert_eq!(body.as_ref(), "olá".as_bytes());
        assert_eq!(ct, None);
    }

    #[rstest]
    #[case::integer(json!(42), b"42" as &[u8])]
    #[case::float(json!(1.5), b"1.5")]
    #[case::boolean(json!(true), b"true")]
    #[case::array(json!([1, 2, 3]), b"[1,2,3]")]
    fn json_value_to_body_non_string_uses_json_content_type(
        #[case] value: JsonValue,
        #[case] expected_bytes: &[u8],
    ) {
        let (body, ct) = json_value_to_body(value).expect("ok");
        assert_eq!(body.as_ref(), expected_bytes);
        assert_eq!(ct, Some(JSON_CONTENT_TYPE));
    }

    #[test]
    fn json_value_to_body_rejects_null() {
        let err = json_value_to_body(JsonValue::Null).expect_err("null must fail");
        assert_eq!(err.status(), StatusCode::BAD_REQUEST);
    }
}
