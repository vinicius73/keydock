use keydock_testkit::TestContext;

#[tokio::test]
async fn metrics_endpoint_includes_http_metrics_after_requests() {
    let ctx = TestContext::new();
    let _ = ctx.server.get("/health").await;

    let response = ctx.server.get("/metrics").await;
    response.assert_status_ok();
    let body = response.text();
    assert!(body.contains("http_requests_total"));
    assert!(body.contains("http_request_duration_seconds"));
}
