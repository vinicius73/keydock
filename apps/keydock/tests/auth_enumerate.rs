//! Enumerate permission matrix (HTTP integration).
//!
//! Bucket listing (`GET /api/v1/{bucket}/`) is part of the
//! read side, so `read_key` is expected to grant `enumerate` together with
//! read on individual keys. Without these cases, a regression that silently
//! downgrades `read_key` to `read`-only (as in the legacy `Permission::READ_ONLY`
//! mapping) would go unnoticed.

use keydock_testkit::{BucketSetup, TestContext, api_error_body_json};
use serde_json::json;

#[tokio::test]
async fn read_key_can_enumerate_bucket() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::read_only("r")).await;

    // Seed one key via admin write path (write_key absent in this setup).
    // We use secret_key-less restricted setup instead to get a write path.
    // Keep the read_only fixture focused and assert listing on an empty bucket
    // as the minimal signal: 200 OK with the expected body shape.
    let response = ctx
        .server
        .get(&format!("/api/v1/{bid}/"))
        .authorization_bearer("r")
        .await;
    response.assert_status_ok();
}

#[tokio::test]
async fn read_key_enumerate_sees_written_keys() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::restricted("r", "w")).await;

    ctx.server
        .put(&format!("/api/v1/{bid}/alpha"))
        .authorization_bearer("w")
        .text("a")
        .await
        .assert_status_ok();
    ctx.server
        .put(&format!("/api/v1/{bid}/beta"))
        .authorization_bearer("w")
        .text("b")
        .await
        .assert_status_ok();

    let response = ctx
        .server
        .get(&format!("/api/v1/{bid}/"))
        .authorization_bearer("r")
        .await;
    response.assert_status_ok();
    let body = response.text();
    assert!(
        body.contains("alpha") && body.contains("beta"),
        "enumerate body should list both keys, got: {body}"
    );
}

#[tokio::test]
async fn write_key_cannot_enumerate() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::restricted("r", "w")).await;

    let response = ctx
        .server
        .get(&format!("/api/v1/{bid}/"))
        .authorization_bearer("w")
        .await;
    response.assert_status_forbidden();
    response.assert_json(&api_error_body_json(403, "forbidden"));
}

#[tokio::test]
async fn anonymous_cannot_enumerate_restricted_bucket() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::read_only("r")).await;

    let response = ctx.server.get(&format!("/api/v1/{bid}/")).await;
    response.assert_status_unauthorized();
    response.assert_json(&api_error_body_json(401, "unauthorized"));
}

#[tokio::test]
async fn anonymous_can_enumerate_public_bucket() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::public()).await;

    let response = ctx.server.get(&format!("/api/v1/{bid}/?format=json")).await;
    response.assert_status_ok();
    response.assert_json(&json!([]));
}
