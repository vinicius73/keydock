//! Key CRUD (HTTP integration).

use axum::http::header;
use keydock_testkit::{BucketSetup, TestContext};
use rstest::rstest;
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
async fn put_text_get_roundtrip() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::restricted("r", "w")).await;
    let path = format!("/{bid}/msg");

    let put = ctx
        .server
        .put(&path)
        .authorization_bearer("w")
        .text("hello")
        .await;
    put.assert_status_ok();
    put.assert_header(header::CONTENT_TYPE, "text/plain; charset=utf-8");
    put.assert_text("hello");

    let get = ctx.server.get(&path).authorization_bearer("r").await;
    get.assert_status_ok();
    get.assert_header(header::CONTENT_TYPE, "text/plain; charset=utf-8");
    get.assert_text("hello");
}

#[tokio::test]
async fn put_json_get_roundtrip() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::restricted("r", "w")).await;
    let path = format!("/{bid}/j1");
    let body = r#"{"x":1}"#;

    let put = ctx
        .server
        .put(&path)
        .authorization_bearer("w")
        .content_type("application/json")
        .text(body)
        .await;
    put.assert_status_ok();
    put.assert_header(header::CONTENT_TYPE, "application/json");
    put.assert_text(body);

    let get = ctx.server.get(&path).authorization_bearer("r").await;
    get.assert_status_ok();
    get.assert_header(header::CONTENT_TYPE, "application/json");
    get.assert_text(body);
}

#[rstest]
#[case("42", "42")]
#[case("3.14", "3.14")]
#[tokio::test]
async fn put_numeric_string_get_roundtrip(#[case] stored: &str, #[case] read_back: &str) {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::restricted("r", "w")).await;
    let path = format!("/{bid}/num");

    ctx.server
        .put(&path)
        .authorization_bearer("w")
        .text(stored)
        .await
        .assert_status_ok();

    let get = ctx.server.get(&path).authorization_bearer("r").await;
    get.assert_status_ok();
    get.assert_header(header::CONTENT_TYPE, "text/plain; charset=utf-8");
    get.assert_text(read_back);
}

#[tokio::test]
async fn get_missing_returns_404() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::read_only("r")).await;
    let response = ctx
        .server
        .get(&format!("/{bid}/nope"))
        .authorization_bearer("r")
        .await;
    response.assert_status_not_found();
    response.assert_json(&err_json(404, "not_found"));
}

#[tokio::test]
async fn post_primary_method_roundtrip() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::restricted("r", "w")).await;
    let path = format!("/{bid}/via-post");

    ctx.server
        .post(&path)
        .authorization_bearer("w")
        .text("data")
        .await
        .assert_status_ok();

    let get = ctx.server.get(&path).authorization_bearer("r").await;
    get.assert_status_ok();
    get.assert_text("data");
}

#[tokio::test]
async fn key_too_long_returns_400() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::admin("sec")).await;
    let long_key = "a".repeat(129);
    let response = ctx
        .server
        .put(&format!("/{bid}/{long_key}"))
        .authorization_bearer("sec")
        .await;
    response.assert_status_bad_request();
    response.assert_json(&err_json(400, "bad_request"));
}

#[tokio::test]
async fn value_too_large_returns_400() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::admin("sec")).await;
    let payload = vec![b'x'; 16 * 1024 + 1];
    let response = ctx
        .server
        .put(&format!("/{bid}/big"))
        .authorization_bearer("sec")
        .bytes(payload.into())
        .await;
    response.assert_status_bad_request();
    response.assert_json(&err_json(400, "bad_request"));
}

#[tokio::test]
async fn delete_existing_then_missing() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::admin("sec")).await;
    let path = format!("/{bid}/delme");

    ctx.server
        .put(&path)
        .authorization_bearer("sec")
        .text("x")
        .await
        .assert_status_ok();

    let del = ctx.server.delete(&path).authorization_bearer("sec").await;
    del.assert_status_no_content();

    let get = ctx.server.get(&path).authorization_bearer("sec").await;
    get.assert_status_not_found();
    get.assert_json(&err_json(404, "not_found"));

    let del_again = ctx.server.delete(&path).authorization_bearer("sec").await;
    del_again.assert_status_not_found();
    del_again.assert_json(&err_json(404, "not_found"));
}

#[tokio::test]
async fn put_without_credential_on_restricted_bucket() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::restricted("r", "w")).await;
    let response = ctx.server.put(&format!("/{bid}/k")).await;
    response.assert_status_unauthorized();
    response.assert_json(&err_json(401, "unauthorized"));
}

#[tokio::test]
async fn put_with_wrong_credential_on_admin_bucket() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::admin("sec")).await;
    let response = ctx
        .server
        .put(&format!("/{bid}/k"))
        .authorization_bearer("wrong")
        .await;
    response.assert_status_unauthorized();
    response.assert_json(&err_json(401, "unauthorized"));
}
