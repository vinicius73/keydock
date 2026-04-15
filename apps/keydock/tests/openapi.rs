//! OpenAPI JSON and Swagger UI integration tests.

use keydock_testkit::TestContext;
use serde_json::Value;

#[tokio::test]
async fn openapi_json_returns_openapi_document() {
    let ctx = TestContext::new();
    let response = ctx.server.get("/api-docs/openapi.json").await;
    response.assert_status_ok();
    let body: Value = response.json();
    assert!(body.get("openapi").is_some());
    assert!(body.get("paths").is_some());
}

#[tokio::test]
async fn swagger_ui_serves_html() {
    let ctx = TestContext::new();
    let response = ctx.server.get("/swagger-ui/").await;
    response.assert_status_ok();
    let body = response.text();
    assert!(
        body.contains("swagger") || body.contains("Swagger"),
        "expected Swagger UI HTML"
    );
}
