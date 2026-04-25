//! OpenAPI JSON and Swagger UI integration tests.

use keydock_testkit::TestContext;
use pretty_assertions::assert_eq;
use serde_json::Value;

#[tokio::test]
async fn openapi_json_returns_openapi_document() {
    let ctx = TestContext::new();
    let response = ctx.server.get("/api-docs/openapi.json").await;
    response.assert_status_ok();
    let body: Value = response.json();
    assert_eq!(body.get("openapi").is_some(), true);
    let paths = body
        .get("paths")
        .and_then(Value::as_object)
        .expect("OpenAPI paths object");
    assert_eq!(paths.contains_key("/api/v1"), true);
    assert_eq!(paths.contains_key("/api/v1/{bucket}"), true);
    assert_eq!(paths.contains_key("/health"), true);
    assert_eq!(paths.contains_key("/ready"), true);
}

#[tokio::test]
async fn swagger_ui_serves_html() {
    let ctx = TestContext::new();
    let response = ctx.server.get("/swagger-ui/").await;
    response.assert_status_ok();
    let body = response.text();
    let has_swagger = body.contains("swagger") || body.contains("Swagger");
    assert_eq!(has_swagger, true);
}
