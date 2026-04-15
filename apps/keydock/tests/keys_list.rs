//! Bucket key listing (`GET /{bucket}/`).

use axum::http::StatusCode;
use axum::http::header;
use keydock_testkit::{BucketSetup, TestContext};
use pretty_assertions::assert_eq;
use serde_json::json;

fn err_json(code: u16, msg: &str) -> serde_json::Value {
    json!({
        "error": {
            "code": code,
            "message": msg
        }
    })
}

#[tokio::test]
async fn list_empty_bucket_json() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::public()).await;
    let list = ctx
        .server
        .get(&format!("/{bid}/"))
        .add_header(header::ACCEPT, "application/json")
        .await;
    list.assert_status_ok();
    list.assert_header(header::CONTENT_TYPE, "application/json");
    list.assert_json(&json!([]));
}

#[tokio::test]
async fn list_keys_lexicographic_json() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::public()).await;
    for k in ["c", "a", "b"] {
        let p = format!("/{bid}/{k}");
        ctx.server.post(&p).text("1").await.assert_status_ok();
    }
    let list = ctx
        .server
        .get(&format!("/{bid}/"))
        .add_header(header::ACCEPT, "application/json")
        .await;
    list.assert_status_ok();
    list.assert_json(&json!(["a", "b", "c"]));
}

#[tokio::test]
async fn list_reverse_true() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::public()).await;
    for k in ["a", "b", "c"] {
        ctx.server
            .post(&format!("/{bid}/{k}"))
            .text("1")
            .await
            .assert_status_ok();
    }
    let list = ctx
        .server
        .get(&format!("/{bid}/?reverse=true"))
        .add_header(header::ACCEPT, "application/json")
        .await;
    list.assert_status_ok();
    list.assert_json(&json!(["c", "b", "a"]));
}

#[tokio::test]
async fn list_prefix_filter() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::public()).await;
    for k in ["foo:1", "foo:2", "bar:1"] {
        ctx.server
            .post(&format!("/{bid}/{k}"))
            .text("x")
            .await
            .assert_status_ok();
    }
    let list = ctx
        .server
        .get(&format!("/{bid}/?prefix=foo%3A"))
        .add_header(header::ACCEPT, "application/json")
        .await;
    list.assert_status_ok();
    list.assert_json(&json!(["foo:1", "foo:2"]));
}

#[tokio::test]
async fn list_skip_limit() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::public()).await;
    for k in ["k0", "k1", "k2", "k3"] {
        ctx.server
            .post(&format!("/{bid}/{k}"))
            .text("1")
            .await
            .assert_status_ok();
    }
    let list = ctx
        .server
        .get(&format!("/{bid}/?limit=2&skip=1"))
        .add_header(header::ACCEPT, "application/json")
        .await;
    list.assert_status_ok();
    list.assert_json(&json!(["k1", "k2"]));
}

#[tokio::test]
async fn list_values_text_format() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::public()).await;
    ctx.server
        .post(&format!("/{bid}/k1"))
        .text("hello")
        .await
        .assert_status_ok();
    let list = ctx
        .server
        .get(&format!("/{bid}/?values=true&format=text"))
        .await;
    list.assert_status_ok();
    list.assert_header(header::CONTENT_TYPE, "text/plain; charset=utf-8");
    list.assert_text("k1=hello");
}

#[tokio::test]
async fn list_values_json_native_json_value() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::public()).await;
    let body = r#"{"a":1}"#;
    ctx.server
        .post(&format!("/{bid}/jk"))
        .content_type("application/json")
        .text(body)
        .await
        .assert_status_ok();

    let list = ctx
        .server
        .get(&format!("/{bid}/?values=true&format=json"))
        .await;
    list.assert_status_ok();
    list.assert_header(header::CONTENT_TYPE, "application/json");
    let v: serde_json::Value = list.json();
    assert_eq!(v, json!([["jk", {"a": 1}]]));
}

#[tokio::test]
async fn list_values_jsonl() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::public()).await;
    ctx.server
        .post(&format!("/{bid}/x"))
        .text("42")
        .await
        .assert_status_ok();

    let list = ctx
        .server
        .get(&format!("/{bid}/?values=true&format=jsonl"))
        .await;
    list.assert_status_ok();
    list.assert_header(header::CONTENT_TYPE, "application/x-ndjson");
    let text = list.text();
    assert_eq!(text.trim(), r#"["x",42]"#);
}

#[tokio::test]
async fn list_invalid_format_returns_406() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::public()).await;
    let res = ctx
        .server
        .get(&format!("/{bid}/?format=not-a-format"))
        .await;
    res.assert_status(StatusCode::NOT_ACCEPTABLE);
    res.assert_json(&err_json(406, "not_acceptable"));
}

#[tokio::test]
async fn list_restricted_bucket_anonymous_returns_401() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::restricted("r", "w")).await;
    let res = ctx.server.get(&format!("/{bid}/")).await;
    res.assert_status_unauthorized();
    res.assert_json(&err_json(401, "unauthorized"));
}

#[tokio::test]
async fn list_read_key_returns_403_forbidden() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::read_only("r")).await;
    let res = ctx
        .server
        .get(&format!("/{bid}/"))
        .authorization_bearer("r")
        .await;
    res.assert_status_forbidden();
    res.assert_json(&err_json(403, "forbidden"));
}
