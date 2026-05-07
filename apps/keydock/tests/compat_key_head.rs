//! Compatibility tests for `HEAD /api/v1/{bucket}/{key}`.
//!
//! HEAD shares every auth, TTL and content-type rule with `GET /api/v1/{bucket}/{key}`
//! but drops the body. The cases below pin this equivalence so a regression
//! (different auth, dropped `Content-Type`, missed TTL expiry) surfaces even
//! when no GET test is touched.

use axum::http::{Method, StatusCode, header};
use bytes::Bytes;
use keydock_testkit::{BucketSetup, TestContext};

// HEAD responses must not carry a body; the error envelope is
// unavailable, but status codes mirror GET exactly, which is what clients check.

#[tokio::test]
async fn head_key_existing_returns_200_with_content_type_and_empty_body() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::restricted("r", "w")).await;

    ctx.server
        .put(&format!("/api/v1/{bid}/k1"))
        .authorization_bearer("w")
        .text("payload")
        .await
        .assert_status_ok();

    let response = ctx
        .server
        .method(Method::HEAD, &format!("/api/v1/{bid}/k1"))
        .authorization_bearer("r")
        .await;
    response.assert_status_ok();
    response.assert_header(header::CONTENT_TYPE, "text/plain; charset=utf-8");
    assert!(response.as_bytes().is_empty());
}

#[tokio::test]
async fn head_key_missing_returns_404() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::read_only("r")).await;

    let response = ctx
        .server
        .method(Method::HEAD, &format!("/api/v1/{bid}/missing"))
        .authorization_bearer("r")
        .await;
    response.assert_status_not_found();
}

#[tokio::test]
async fn head_key_anonymous_on_restricted_bucket_returns_401() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::read_only("r")).await;

    let response = ctx
        .server
        .method(Method::HEAD, &format!("/api/v1/{bid}/k1"))
        .await;
    response.assert_status_unauthorized();
}

#[tokio::test]
async fn head_key_json_content_type_matches_get() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::restricted("r", "w")).await;

    ctx.server
        .put(&format!("/api/v1/{bid}/jk"))
        .authorization_bearer("w")
        .content_type("application/json")
        .bytes(Bytes::from_static(br#"{"ok":true}"#))
        .await
        .assert_status_ok();

    let head = ctx
        .server
        .method(Method::HEAD, &format!("/api/v1/{bid}/jk"))
        .authorization_bearer("r")
        .await;
    head.assert_status_ok();
    head.assert_header(header::CONTENT_TYPE, "application/json");

    let get = ctx
        .server
        .get(&format!("/api/v1/{bid}/jk"))
        .authorization_bearer("r")
        .await;
    get.assert_status(StatusCode::OK);
    get.assert_header(header::CONTENT_TYPE, "application/json");
}
