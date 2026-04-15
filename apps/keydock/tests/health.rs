use serde_json::Value;

#[tokio::test]
async fn health_returns_ok() {
    let (_dir, server) = keydock_testkit::test_app();
    let response = server.get("/health").await;
    response.assert_status_ok();
    let body: Value = response.json();
    assert_eq!(body["status"], "ok");
    assert_eq!(body["version"], "0.1.0-alpha");
}
