//! Multi-key transactions (`POST /{bucket}`) — HTTP integration.

use axum::http::header;
use bytes::Bytes;
use rstest::rstest;
use serde_json::{Value, json};
use tokio::time::{Duration, sleep};

use keydock_testkit::{BucketSetup, TestContext, TokenSetup, api_error_body_json};

#[tokio::test]
async fn txn_empty_batch_returns_204() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::restricted("r", "w")).await;

    let res = ctx
        .server
        .post(&format!("/{bid}"))
        .authorization_bearer("w")
        .json(&json!({ "txn": [] }))
        .await;
    res.assert_status_no_content();
}

#[tokio::test]
async fn txn_key_percent_decodes_like_path_keys() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::restricted("r", "w")).await;

    let res = ctx
        .server
        .post(&format!("/{bid}"))
        .authorization_bearer("w")
        .json(&json!({
            "txn": [{ "set": "hello%20world", "value": "ok" }]
        }))
        .await;
    res.assert_status_no_content();

    let get = ctx
        .server
        .get(&format!("/{bid}/hello%20world"))
        .authorization_bearer("r")
        .await;
    get.assert_status_ok();
    get.assert_text("ok");
}

#[tokio::test]
async fn txn_set_creates_key() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::restricted("r", "w")).await;

    let res = ctx
        .server
        .post(&format!("/{bid}"))
        .authorization_bearer("w")
        .json(&json!({
            "txn": [{ "set": "k", "value": "hello" }]
        }))
        .await;
    res.assert_status_no_content();

    let get = ctx
        .server
        .get(&format!("/{bid}/k"))
        .authorization_bearer("r")
        .await;
    get.assert_status_ok();
    get.assert_header(header::CONTENT_TYPE, "text/plain; charset=utf-8");
    get.assert_text("hello");
}

#[tokio::test]
async fn txn_delete_removes_key() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::admin("sec")).await;

    ctx.server
        .put(&format!("/{bid}/k"))
        .authorization_bearer("sec")
        .text("x")
        .await
        .assert_status_ok();

    let res = ctx
        .server
        .post(&format!("/{bid}"))
        .authorization_bearer("sec")
        .json(&json!({
            "txn": [{ "delete": "k" }]
        }))
        .await;
    res.assert_status_no_content();

    let get = ctx
        .server
        .get(&format!("/{bid}/k"))
        .authorization_bearer("sec")
        .await;
    get.assert_status_not_found();
    get.assert_json(&api_error_body_json(404, "not_found"));
}

#[tokio::test]
async fn txn_set_and_delete_atomic() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::admin("sec")).await;

    ctx.server
        .put(&format!("/{bid}/k2"))
        .authorization_bearer("sec")
        .text("v")
        .await
        .assert_status_ok();

    let res = ctx
        .server
        .post(&format!("/{bid}"))
        .authorization_bearer("sec")
        .json(&json!({
            "txn": [
                { "set": "k1", "value": "a" },
                { "delete": "k2" }
            ]
        }))
        .await;
    res.assert_status_no_content();

    let g1 = ctx
        .server
        .get(&format!("/{bid}/k1"))
        .authorization_bearer("sec")
        .await;
    g1.assert_status_ok();
    g1.assert_text("a");

    let g2 = ctx
        .server
        .get(&format!("/{bid}/k2"))
        .authorization_bearer("sec")
        .await;
    g2.assert_status_not_found();
    g2.assert_json(&api_error_body_json(404, "not_found"));
}

#[tokio::test]
async fn txn_set_with_ttl_expires() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::restricted("r", "w")).await;

    let res = ctx
        .server
        .post(&format!("/{bid}"))
        .authorization_bearer("w")
        .json(&json!({
            "txn": [{ "set": "kt", "value": "z", "ttl": 1 }]
        }))
        .await;
    res.assert_status_no_content();

    sleep(Duration::from_millis(2100)).await;

    let get = ctx
        .server
        .get(&format!("/{bid}/kt"))
        .authorization_bearer("r")
        .await;
    get.assert_status_not_found();
    get.assert_json(&api_error_body_json(404, "not_found"));
}

#[tokio::test]
async fn txn_invalid_key_too_long_returns_400() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::restricted("r", "w")).await;
    let long_key = "a".repeat(129);

    let res = ctx
        .server
        .post(&format!("/{bid}"))
        .authorization_bearer("w")
        .json(&json!({
            "txn": [{ "set": long_key, "value": "v" }]
        }))
        .await;
    res.assert_status_bad_request();
    res.assert_json(&api_error_body_json(400, "bad_request"));
}

#[tokio::test]
async fn txn_malformed_json_is_rejected() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::restricted("r", "w")).await;

    let res = ctx
        .server
        .post(&format!("/{bid}"))
        .authorization_bearer("w")
        .content_type("application/json")
        .bytes(Bytes::from("not-json"))
        .await;
    res.assert_status_bad_request();
    res.assert_json(&api_error_body_json(400, "bad_request"));
}

#[tokio::test]
async fn txn_requires_write_for_set() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::restricted("r", "w")).await;

    let res = ctx
        .server
        .post(&format!("/{bid}"))
        .authorization_bearer("r")
        .json(&json!({
            "txn": [{ "set": "k", "value": "x" }]
        }))
        .await;
    res.assert_status_forbidden();
    res.assert_json(&api_error_body_json(403, "forbidden"));
}

#[tokio::test]
async fn txn_requires_delete_permission() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::restricted("r", "w")).await;

    let res = ctx
        .server
        .post(&format!("/{bid}"))
        .authorization_bearer("w")
        .json(&json!({
            "txn": [{ "delete": "k" }]
        }))
        .await;
    res.assert_status_forbidden();
    res.assert_json(&api_error_body_json(403, "forbidden"));
}

#[tokio::test]
async fn txn_scoped_token_prefix_enforced_on_set() {
    let ctx = TestContext::new();
    let bid = ctx
        .create_bucket(BucketSetup {
            secret_key: Some("sec".into()),
            signing_key: Some("sign".into()),
            ..BucketSetup::default()
        })
        .await;

    let form = TokenSetup {
        prefix: "x:".into(),
        permissions: "read,write,enumerate,delete".into(),
        ttl: 3600,
    };
    let tok = ctx.create_token(&bid, "sec", &form).await;
    tok.assert_status_ok();
    let access: serde_json::Value = tok.json();
    let token = access
        .get("access_token")
        .and_then(|v| v.as_str())
        .expect("access_token");

    let res = ctx
        .server
        .post(&format!("/{bid}"))
        .authorization_bearer(token)
        .json(&json!({
            "txn": [{ "set": "y:k", "value": "1" }]
        }))
        .await;
    res.assert_status_forbidden();
    res.assert_json(&api_error_body_json(403, "forbidden"));
}

#[tokio::test]
async fn txn_no_partial_mutation_when_later_op_fails_authz() {
    let ctx = TestContext::new();
    let bid = ctx
        .create_bucket(BucketSetup {
            secret_key: Some("sec".into()),
            signing_key: Some("sign".into()),
            ..BucketSetup::default()
        })
        .await;

    ctx.server
        .put(&format!("/{bid}/scope:seed"))
        .authorization_bearer("sec")
        .text("orig")
        .await
        .assert_status_ok();

    // Scoped token without `delete`; prefix is required and non-empty,
    // so we co-locate both keys under `scope:` and restrict the token there.
    let form = TokenSetup {
        prefix: "scope:".into(),
        permissions: "read,write,enumerate".into(),
        ttl: 3600,
    };
    let tok = ctx.create_token(&bid, "sec", &form).await;
    tok.assert_status_ok();
    let access: serde_json::Value = tok.json();
    let token = access
        .get("access_token")
        .and_then(|v| v.as_str())
        .expect("access_token");

    let res = ctx
        .server
        .post(&format!("/{bid}"))
        .authorization_bearer(token)
        .json(&json!({
            "txn": [
                { "set": "scope:newk", "value": "nv" },
                { "delete": "scope:seed" }
            ]
        }))
        .await;
    res.assert_status_forbidden();
    res.assert_json(&api_error_body_json(403, "forbidden"));

    let g_new = ctx
        .server
        .get(&format!("/{bid}/scope:newk"))
        .authorization_bearer(token)
        .await;
    g_new.assert_status_not_found();

    let g_seed = ctx
        .server
        .get(&format!("/{bid}/scope:seed"))
        .authorization_bearer(token)
        .await;
    g_seed.assert_status_ok();
    g_seed.assert_text("orig");
}

/// JSON scalar values (`Number`, `Bool`) must be stored as `application/json`
/// so the original JSON type survives round-trip, instead of being re-inferred
/// as Int64/Float64 from the serialized bytes.
#[rstest]
#[case::integer(json!(42), "42")]
#[case::float(json!(1.5), "1.5")]
#[case::boolean(json!(true), "true")]
#[tokio::test]
async fn txn_set_value_json_scalar_becomes_json(#[case] value: Value, #[case] expected_body: &str) {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::restricted("r", "w")).await;

    let res = ctx
        .server
        .post(&format!("/{bid}"))
        .authorization_bearer("w")
        .json(&json!({
            "txn": [{ "set": "k", "value": value }]
        }))
        .await;
    res.assert_status_no_content();

    let get = ctx
        .server
        .get(&format!("/{bid}/k"))
        .authorization_bearer("r")
        .await;
    get.assert_status_ok();
    get.assert_header(header::CONTENT_TYPE, "application/json");
    get.assert_text(expected_body);
}

#[tokio::test]
async fn txn_set_value_object_becomes_json() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::restricted("r", "w")).await;
    let payload = json!({ "a": 1, "b": "x" });

    let res = ctx
        .server
        .post(&format!("/{bid}"))
        .authorization_bearer("w")
        .json(&json!({
            "txn": [{ "set": "o", "value": payload }]
        }))
        .await;
    res.assert_status_no_content();

    let get = ctx
        .server
        .get(&format!("/{bid}/o"))
        .authorization_bearer("r")
        .await;
    get.assert_status_ok();
    get.assert_header(header::CONTENT_TYPE, "application/json");
    get.assert_json(&json!({ "a": 1, "b": "x" }));
}

#[tokio::test]
async fn txn_set_value_array_becomes_json() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::restricted("r", "w")).await;

    let res = ctx
        .server
        .post(&format!("/{bid}"))
        .authorization_bearer("w")
        .json(&json!({
            "txn": [{ "set": "a", "value": [1, 2, 3] }]
        }))
        .await;
    res.assert_status_no_content();

    let get = ctx
        .server
        .get(&format!("/{bid}/a"))
        .authorization_bearer("r")
        .await;
    get.assert_status_ok();
    get.assert_header(header::CONTENT_TYPE, "application/json");
    get.assert_json(&json!([1, 2, 3]));
}

/// Strings fall through to the default inference: non-numeric UTF-8 ends up as
/// `text/plain; charset=utf-8` (ValueKind::Utf8).
#[tokio::test]
async fn txn_set_value_utf8_string_preserved() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::restricted("r", "w")).await;

    let res = ctx
        .server
        .post(&format!("/{bid}"))
        .authorization_bearer("w")
        .json(&json!({
            "txn": [{ "set": "s", "value": "olá" }]
        }))
        .await;
    res.assert_status_no_content();

    let get = ctx
        .server
        .get(&format!("/{bid}/s"))
        .authorization_bearer("r")
        .await;
    get.assert_status_ok();
    get.assert_header(header::CONTENT_TYPE, "text/plain; charset=utf-8");
    get.assert_text("olá");
}

/// A numeric *string* (JSON `"42"`) must not be promoted to JSON; it stays as
/// a plain-text integer. Anchors the distinction against `json!(42)`.
#[tokio::test]
async fn txn_set_value_numeric_string_stays_plaintext() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::restricted("r", "w")).await;

    let res = ctx
        .server
        .post(&format!("/{bid}"))
        .authorization_bearer("w")
        .json(&json!({
            "txn": [{ "set": "n", "value": "42" }]
        }))
        .await;
    res.assert_status_no_content();

    let get = ctx
        .server
        .get(&format!("/{bid}/n"))
        .authorization_bearer("r")
        .await;
    get.assert_status_ok();
    get.assert_header(header::CONTENT_TYPE, "text/plain; charset=utf-8");
    get.assert_text("42");
}

/// Invalid item shapes must be rejected with the canonical 400 envelope
/// before any mutation is attempted. Covers: explicit JSON `null` value,
/// the legacy `cmd` shape, and ambiguous items carrying both `set` and
/// `delete` keys.
#[rstest]
#[case::null_value(json!({"txn":[{"set":"k","value":null}]}))]
#[case::missing_value(json!({"txn":[{"set":"k"}]}))]
#[case::legacy_cmd_shape(json!({"txn":[{"cmd":"set","key":"k","value":"v"}]}))]
#[case::both_set_and_delete(json!({"txn":[{"set":"a","delete":"b","value":"v"}]}))]
#[case::unknown_extra_field(json!({"txn":[{"set":"k","value":"v","ttl":1,"extra":true}]}))]
#[tokio::test]
async fn txn_rejects_invalid_item_shapes(#[case] payload: Value) {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::restricted("r", "w")).await;

    let res = ctx
        .server
        .post(&format!("/{bid}"))
        .authorization_bearer("w")
        .json(&payload)
        .await;
    res.assert_status_bad_request();
    res.assert_json(&api_error_body_json(400, "bad_request"));
}
