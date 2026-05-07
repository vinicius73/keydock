use axum::http::StatusCode;

use serde_json::json;

use keydock_testkit::TestContext;

#[tokio::test]
async fn health_returns_ok() {
    let ctx = TestContext::new();
    let response = ctx.server.get("/health").await;
    response.assert_status_ok();
    response.assert_json(&json!({
        "status": "ok",
        "storage": "ok",
        "version": "0.1.0-alpha"
    }));
}

#[tokio::test]
async fn ready_returns_ok_when_storage_up() {
    let ctx = TestContext::new();
    let response = ctx.server.get("/ready").await;
    response.assert_status_ok();
    response.assert_json(&json!({
        "status": "ok",
        "storage": "ok",
        "version": "0.1.0-alpha"
    }));
}

#[tokio::test]
async fn ready_returns_503_when_metadata_ping_fails() {
    let ctx = TestContext::new();
    ctx.testkit_set_fail_ping_metadata(true);

    let degraded = ctx.server.get("/ready").await;
    degraded.assert_status(StatusCode::SERVICE_UNAVAILABLE);
    degraded.assert_json(&json!({
        "status": "degraded",
        "storage": "error",
        "version": "0.1.0-alpha"
    }));

    ctx.testkit_set_fail_ping_metadata(false);

    let recovered = ctx.server.get("/ready").await;
    recovered.assert_status_ok();
    recovered.assert_json(&json!({
        "status": "ok",
        "storage": "ok",
        "version": "0.1.0-alpha"
    }));
}
