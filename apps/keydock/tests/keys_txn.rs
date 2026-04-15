//! Multi-key transactions (`POST /{bucket}`) — HTTP integration.

use axum::http::header;
use bytes::Bytes;
use serde_json::json;
use tokio::time::{Duration, sleep};

use keydock_testkit::{BucketSetup, TestContext, TokenSetup};

fn err_json(code: u16, msg: &str) -> serde_json::Value {
    json!({
        "error": {
            "code": code,
            "message": msg
        }
    })
}

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
async fn txn_set_creates_key() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::restricted("r", "w")).await;

    let res = ctx
        .server
        .post(&format!("/{bid}"))
        .authorization_bearer("w")
        .json(&json!({
            "txn": [{ "cmd": "set", "key": "k", "value": "hello" }]
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
            "txn": [{ "cmd": "delete", "key": "k" }]
        }))
        .await;
    res.assert_status_no_content();

    let get = ctx
        .server
        .get(&format!("/{bid}/k"))
        .authorization_bearer("sec")
        .await;
    get.assert_status_not_found();
    get.assert_json(&err_json(404, "not_found"));
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
                { "cmd": "set", "key": "k1", "value": "a" },
                { "cmd": "delete", "key": "k2" }
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
    g2.assert_json(&err_json(404, "not_found"));
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
            "txn": [{ "cmd": "set", "key": "kt", "value": "z", "ttl": 1 }]
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
    get.assert_json(&err_json(404, "not_found"));
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
            "txn": [{ "cmd": "set", "key": long_key, "value": "v" }]
        }))
        .await;
    res.assert_status_bad_request();
    res.assert_json(&err_json(400, "bad_request"));
}

#[tokio::test]
async fn txn_set_missing_value_returns_400() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::restricted("r", "w")).await;

    let res = ctx
        .server
        .post(&format!("/{bid}"))
        .authorization_bearer("w")
        .json(&json!({
            "txn": [{ "cmd": "set", "key": "k" }]
        }))
        .await;
    res.assert_status_bad_request();
    res.assert_json(&err_json(400, "bad_request"));
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
    res.assert_text_contains("Failed to parse the request body as JSON");
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
            "txn": [{ "cmd": "set", "key": "k", "value": "x" }]
        }))
        .await;
    res.assert_status_forbidden();
    res.assert_json(&err_json(403, "forbidden"));
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
            "txn": [{ "cmd": "delete", "key": "k" }]
        }))
        .await;
    res.assert_status_forbidden();
    res.assert_json(&err_json(403, "forbidden"));
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
        prefix: Some("x:".into()),
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
            "txn": [{ "cmd": "set", "key": "y:k", "value": "1" }]
        }))
        .await;
    res.assert_status_forbidden();
    res.assert_json(&err_json(403, "forbidden"));
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
        .put(&format!("/{bid}/seed"))
        .authorization_bearer("sec")
        .text("orig")
        .await
        .assert_status_ok();

    let form = TokenSetup {
        prefix: None,
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
                { "cmd": "set", "key": "newk", "value": "nv" },
                { "cmd": "delete", "key": "seed" }
            ]
        }))
        .await;
    res.assert_status_forbidden();
    res.assert_json(&err_json(403, "forbidden"));

    let g_new = ctx
        .server
        .get(&format!("/{bid}/newk"))
        .authorization_bearer(token)
        .await;
    g_new.assert_status_not_found();

    let g_seed = ctx
        .server
        .get(&format!("/{bid}/seed"))
        .authorization_bearer(token)
        .await;
    g_seed.assert_status_ok();
    g_seed.assert_text("orig");
}
