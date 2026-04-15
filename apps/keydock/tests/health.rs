use serde_json::json;

#[tokio::test]
async fn health_returns_ok() {
    let (_dir, server) = keydock_testkit::test_app().expect("test_app");
    let response = server.get("/health").await;
    response.assert_status_ok();
    response.assert_json(&json!({
        "status": "ok",
        "version": "0.1.0-alpha"
    }));
}
