//! Compatibility tests for `HEAD /{bucket}`.
//!
//! HEAD shares auth with `GET /{bucket}` (admin-only) and returns an empty
//! body so callers can probe existence without downloading the policy view.
//! Without these anchors, a silent regression to 404 or 403 would go
//! unnoticed because no existing test exercises the HEAD method.

use axum::http::Method;
use keydock_testkit::{BucketSetup, TestContext};
use uuid::Uuid;

// HEAD responses must not carry a body, so error envelopes
// are not available here; only the status code is meaningful, and that is
// exactly what downstream clients rely on.

#[tokio::test]
async fn head_bucket_as_admin_returns_200_with_empty_body() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::admin("sec")).await;

    let response = ctx
        .server
        .method(Method::HEAD, &format!("/{bid}"))
        .authorization_bearer("sec")
        .await;
    response.assert_status_ok();
    assert!(response.as_bytes().is_empty());
}

#[tokio::test]
async fn head_bucket_unknown_returns_404() {
    let ctx = TestContext::new();
    let unknown = Uuid::new_v4().to_string();

    let response = ctx
        .server
        .method(Method::HEAD, &format!("/{unknown}"))
        .await;
    response.assert_status_not_found();
}

#[tokio::test]
async fn head_bucket_non_admin_returns_403() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::restricted("r", "w")).await;

    let response = ctx
        .server
        .method(Method::HEAD, &format!("/{bid}"))
        .authorization_bearer("r")
        .await;
    response.assert_status_forbidden();
}

#[tokio::test]
async fn head_bucket_anonymous_restricted_returns_403() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::admin("sec")).await;

    let response = ctx.server.method(Method::HEAD, &format!("/{bid}")).await;
    response.assert_status_forbidden();
}
