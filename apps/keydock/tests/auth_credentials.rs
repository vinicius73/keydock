//! Credential channel and permission matrix (HTTP integration).

use axum::http::header;

use keydock_testkit::{BucketSetup, TestContext, api_error_body_json, basic_auth_header};

#[tokio::test]
async fn bearer_secret_key_grants_admin() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::admin("sec")).await;

    let response = ctx
        .server
        .put(&format!("/api/v1/{bid}/k1"))
        .authorization_bearer("sec")
        .await;
    response.assert_status_ok();
    response.assert_header(header::CONTENT_TYPE, "text/plain; charset=utf-8");
    response.assert_text("");
}

#[tokio::test]
async fn bearer_write_key_grants_write() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::restricted("r", "w")).await;

    let ok = ctx
        .server
        .put(&format!("/api/v1/{bid}/k1"))
        .authorization_bearer("w")
        .await;
    ok.assert_status_ok();
    ok.assert_header(header::CONTENT_TYPE, "text/plain; charset=utf-8");
    ok.assert_text("");

    let forbidden = ctx
        .server
        .put(&format!("/api/v1/{bid}/k1"))
        .authorization_bearer("r")
        .await;
    forbidden.assert_status_forbidden();
    forbidden.assert_json(&api_error_body_json(403, "forbidden"));
}

#[tokio::test]
async fn bearer_read_key_grants_read() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::read_only("r")).await;

    let ok = ctx
        .server
        .get(&format!("/api/v1/{bid}/k1"))
        .authorization_bearer("r")
        .await;
    ok.assert_status_not_found();
    ok.assert_json(&api_error_body_json(404, "not_found"));

    let unauthorized = ctx.server.get(&format!("/api/v1/{bid}/k1")).await;
    unauthorized.assert_status_unauthorized();
    unauthorized.assert_json(&api_error_body_json(401, "unauthorized"));
}

#[tokio::test]
async fn query_param_access_token_works() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::read_only("r")).await;

    let response = ctx
        .server
        .get(&format!("/api/v1/{bid}/k1?access_token=r"))
        .await;
    response.assert_status_not_found();
    response.assert_json(&api_error_body_json(404, "not_found"));
}

#[tokio::test]
async fn query_param_key_works() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::read_only("r")).await;

    let response = ctx.server.get(&format!("/api/v1/{bid}/k1?key=r")).await;
    response.assert_status_not_found();
    response.assert_json(&api_error_body_json(404, "not_found"));
}

#[tokio::test]
async fn bearer_header_wins_over_access_token_query() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::read_only("r")).await;

    let response = ctx
        .server
        .get(&format!("/api/v1/{bid}/k1?access_token=wrong"))
        .authorization_bearer("r")
        .await;
    response.assert_status_not_found();
    response.assert_json(&api_error_body_json(404, "not_found"));
}

#[tokio::test]
async fn bearer_header_wins_over_key_query() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::read_only("r")).await;

    let response = ctx
        .server
        .get(&format!("/api/v1/{bid}/k1?key=wrong"))
        .authorization_bearer("r")
        .await;
    response.assert_status_not_found();
    response.assert_json(&api_error_body_json(404, "not_found"));
}

#[tokio::test]
async fn access_token_query_wins_over_key_query() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::read_only("r")).await;

    let response = ctx
        .server
        .get(&format!("/api/v1/{bid}/k1?key=wrong&access_token=r"))
        .await;
    response.assert_status_not_found();
    response.assert_json(&api_error_body_json(404, "not_found"));
}

#[tokio::test]
async fn basic_auth_works() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::read_only("r")).await;

    let response = ctx
        .server
        .get(&format!("/api/v1/{bid}/k1"))
        .authorization(basic_auth_header("r"))
        .await;
    response.assert_status_not_found();
    response.assert_json(&api_error_body_json(404, "not_found"));
}

#[tokio::test]
async fn basic_auth_wrong_password_still_authenticates() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::read_only("r")).await;

    let response = ctx
        .server
        .get(&format!("/api/v1/{bid}/k1"))
        .authorization("Basic cjp3cm9uZ3Bhc3N3b3Jk")
        .await;
    response.assert_status_not_found();
    response.assert_json(&api_error_body_json(404, "not_found"));
}

#[tokio::test]
async fn basic_auth_no_colon_uses_full_string() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::read_only("r")).await;

    let response = ctx
        .server
        .get(&format!("/api/v1/{bid}/k1"))
        .authorization("Basic cg==")
        .await;
    response.assert_status_not_found();
    response.assert_json(&api_error_body_json(404, "not_found"));
}

#[tokio::test]
async fn basic_auth_invalid_base64_falls_through_to_query() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::read_only("r")).await;

    let response = ctx
        .server
        .get(&format!("/api/v1/{bid}/k1?access_token=r"))
        .authorization("Basic !!!invalid!!!")
        .await;
    response.assert_status_not_found();
    response.assert_json(&api_error_body_json(404, "not_found"));
}

#[tokio::test]
async fn wrong_credential_returns_401() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::admin("sec")).await;

    let response = ctx
        .server
        .get(&format!("/api/v1/{bid}/k1"))
        .authorization_bearer("wrong")
        .await;
    response.assert_status_unauthorized();
    response.assert_json(&api_error_body_json(401, "unauthorized"));
}

#[tokio::test]
async fn missing_bucket_returns_404() {
    let ctx = TestContext::new();
    let response = ctx.server.get("/api/v1/no-such-bucket/k").await;
    response.assert_status_not_found();
    response.assert_json(&api_error_body_json(404, "not_found"));
}

#[tokio::test]
async fn anonymous_public_bucket_read() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::public()).await;

    let response = ctx.server.get(&format!("/api/v1/{bid}/k1")).await;
    response.assert_status_not_found();
    response.assert_json(&api_error_body_json(404, "not_found"));
}

#[tokio::test]
async fn anonymous_restricted_bucket_read() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::read_only("r")).await;

    let response = ctx.server.get(&format!("/api/v1/{bid}/k1")).await;
    response.assert_status_unauthorized();

    response.assert_json(&api_error_body_json(401, "unauthorized"));
}
