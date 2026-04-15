use keydock_testkit::TestContext;
use serde_json::json;

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
