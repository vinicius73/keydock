//! API rate limiting (`GovernorLayer` + lazy-limit) integration coverage.
//!
//! Throttled responses use axum-governor plain body (`Too Many Requests`), not the JSON
//! [`ErrorBody`](keydock_http::error::ErrorBody) envelope used by application handlers.

use axum::http::{StatusCode, header};

use pretty_assertions::assert_eq;
use serde_json::json;
use serial_test::serial;

use keydock_testkit::{BucketSetup, RateLimitSettings, RouterOptions, TestContext};

/// Rate limiting uses a process-global limiter; keep this module serialized so no
/// other test observes surprising `429` responses mid-suite.
#[serial]
#[tokio::test]
async fn api_returns_429_when_hourly_quota_exhausted() {
    let ctx = TestContext::with_router_options(RouterOptions {
        expose_metrics: true,
        rate_limit: RateLimitSettings {
            enabled: true,
            requests_per_hour: 2,
        },
    })
    .await;

    let bid = ctx.create_bucket(BucketSetup::public()).await;

    let first_put = ctx
        .server
        .put(&format!("/api/v1/{bid}/k1"))
        .add_header("x-real-ip", "203.0.113.1")
        .content_type("text/plain; charset=utf-8")
        .bytes("a".into())
        .await;
    first_put.assert_status_ok();
    first_put.assert_header(header::CONTENT_TYPE, "text/plain; charset=utf-8");
    first_put.assert_text("a");

    let second_put = ctx
        .server
        .put(&format!("/api/v1/{bid}/k2"))
        .add_header("x-real-ip", "203.0.113.1")
        .content_type("text/plain; charset=utf-8")
        .bytes("b".into())
        .await;
    second_put.assert_status(StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(second_put.text(), "Too Many Requests");

    let health = ctx.server.get("/health").await;
    health.assert_status_ok();
    health.assert_json(&json!({
        "status": "ok",
        "storage": "ok",
        "version": "0.1.0-alpha"
    }));
}
