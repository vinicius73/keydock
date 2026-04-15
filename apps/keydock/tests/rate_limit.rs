use keydock_testkit::TestContext;
use pretty_assertions::assert_eq;
use serde_json::json;

#[tokio::test]
async fn rate_limit_allows_n_requests_then_429_with_headers() {
    let ctx = TestContext::with_rate_limit(5);

    for _ in 0..5 {
        let response = ctx.server.get("/health").await;
        response.assert_status_ok();
        response.assert_header("x-ratelimit-limit", "5");
        response.assert_json(&json!({
            "status": "ok",
            "storage": "ok",
            "version": "0.1.0-alpha"
        }));
    }

    let blocked = ctx.server.get("/health").await;
    assert_eq!(blocked.status_code().as_u16(), 429);
    blocked.assert_header("x-ratelimit-limit", "5");
    blocked.assert_header("x-ratelimit-remaining", "0");
    blocked.assert_json(&json!({
        "error": {
            "code": 429,
            "message": "rate limit exceeded"
        }
    }));
}
