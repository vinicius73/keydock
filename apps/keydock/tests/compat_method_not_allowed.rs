//! 405 Method Not Allowed JSON error envelope regression tests.
//!
//! Default Axum fallback returns an empty body for unsupported methods, which
//! breaks the shared `{"error": {...}}` envelope. These tests lock in the
//! `method_not_allowed_fallback` wiring: mounted paths always answer 405 with
//! a JSON body, while non-mounted paths continue to return 404.

use keydock_testkit::{BucketSetup, TestContext, api_error_body_json};

#[tokio::test]
async fn post_on_health_returns_405_with_envelope() {
    let ctx = TestContext::new();
    let response = ctx.server.post("/health").await;
    response.assert_status(axum::http::StatusCode::METHOD_NOT_ALLOWED);
    response.assert_json(&api_error_body_json(405, "method_not_allowed"));
}

#[tokio::test]
async fn delete_on_metrics_returns_405_with_envelope() {
    let ctx = TestContext::new();
    let response = ctx.server.delete("/metrics").await;
    response.assert_status(axum::http::StatusCode::METHOD_NOT_ALLOWED);
    response.assert_json(&api_error_body_json(405, "method_not_allowed"));
}

#[tokio::test]
async fn get_on_root_returns_405_with_envelope() {
    // `POST /` exists (create_bucket), so GET must yield 405 (not 404).
    let ctx = TestContext::new();
    let response = ctx.server.get("/").await;
    response.assert_status(axum::http::StatusCode::METHOD_NOT_ALLOWED);
    response.assert_json(&api_error_body_json(405, "method_not_allowed"));
}

#[tokio::test]
async fn unsupported_method_on_bucket_key_returns_405_with_envelope() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::admin("sec")).await;

    // `POST /{bucket}` is `execute_txn`; OPTIONS is allowed by CORS preflight.
    // We exercise a path that exists but does not register this method.
    let response = ctx
        .server
        .method(axum::http::Method::CONNECT, &format!("/{bid}"))
        .authorization_bearer("sec")
        .await;
    response.assert_status(axum::http::StatusCode::METHOD_NOT_ALLOWED);
    response.assert_json(&api_error_body_json(405, "method_not_allowed"));
}

#[tokio::test]
async fn unknown_path_still_returns_404_not_405() {
    // Paths outside the mounted tree remain 404 (no fallback swap). Important
    // so that clients can distinguish "wrong URL" from "wrong method".
    let ctx = TestContext::new();
    let response = ctx.server.get("/this-is-not-a-route").await;
    response.assert_status_not_found();
}
