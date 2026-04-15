//! Counter / PATCH key (HTTP integration).

use axum::http::header;
use rstest::rstest;
use tokio::time::{Duration, sleep};

use keydock_testkit::{BucketSetup, TestContext, TokenSetup, api_error_body_json};

#[tokio::test]
async fn patch_missing_key_plus_1() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::restricted("r", "w")).await;
    let path = format!("/{bid}/c");

    let patch = ctx
        .server
        .patch(&path)
        .authorization_bearer("w")
        .text("+1")
        .await;
    patch.assert_status_ok();
    patch.assert_header(header::CONTENT_TYPE, "text/plain; charset=utf-8");
    patch.assert_text("1");
}

#[tokio::test]
async fn patch_missing_key_minus_3() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::restricted("r", "w")).await;
    let path = format!("/{bid}/neg");

    let patch = ctx
        .server
        .patch(&path)
        .authorization_bearer("w")
        .text("-3")
        .await;
    patch.assert_status_ok();
    patch.assert_text("-3");
}

#[tokio::test]
async fn patch_int_plus_int() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::restricted("r", "w")).await;
    let path = format!("/{bid}/n");

    ctx.server
        .put(&path)
        .authorization_bearer("w")
        .text("10")
        .await
        .assert_status_ok();

    let patch = ctx
        .server
        .patch(&path)
        .authorization_bearer("w")
        .text("+5")
        .await;
    patch.assert_status_ok();
    patch.assert_text("15");
}

#[tokio::test]
async fn patch_int_plus_float() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::restricted("r", "w")).await;
    let path = format!("/{bid}/pf");

    ctx.server
        .put(&path)
        .authorization_bearer("w")
        .text("10")
        .await
        .assert_status_ok();

    let patch = ctx
        .server
        .patch(&path)
        .authorization_bearer("w")
        .text("+1.5")
        .await;
    patch.assert_status_ok();
    patch.assert_header(header::CONTENT_TYPE, "text/plain; charset=utf-8");
    patch.assert_text("11.5");
}

#[tokio::test]
async fn patch_float_plus_int() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::restricted("r", "w")).await;
    let path = format!("/{bid}/fi");

    ctx.server
        .put(&path)
        .authorization_bearer("w")
        .text("1.5")
        .await
        .assert_status_ok();

    let patch = ctx
        .server
        .patch(&path)
        .authorization_bearer("w")
        .text("+1")
        .await;
    patch.assert_status_ok();
    patch.assert_text("2.5");
}

#[rstest]
#[case::no_sign("5")]
#[case::inf("+Inf")]
#[case::nan("+NaN")]
#[tokio::test]
async fn patch_invalid_body_returns_400(#[case] body: &str) {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::restricted("r", "w")).await;
    let path = format!("/{bid}/bad");

    let patch = ctx
        .server
        .patch(&path)
        .authorization_bearer("w")
        .text(body)
        .await;
    patch.assert_status_bad_request();
    patch.assert_json(&api_error_body_json(400, "bad_request"));
}

#[tokio::test]
async fn patch_non_numeric_existing_value_returns_400() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::restricted("r", "w")).await;
    let path = format!("/{bid}/txt");

    ctx.server
        .put(&path)
        .authorization_bearer("w")
        .text("hello")
        .await
        .assert_status_ok();

    let patch = ctx
        .server
        .patch(&path)
        .authorization_bearer("w")
        .text("+1")
        .await;
    patch.assert_status_bad_request();
    patch.assert_json(&api_error_body_json(400, "bad_request"));
}

#[tokio::test]
async fn patch_overflow_int64_returns_400() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::restricted("r", "w")).await;
    let path = format!("/{bid}/ov");

    ctx.server
        .put(&path)
        .authorization_bearer("w")
        .text(format!("{}", i64::MAX))
        .await
        .assert_status_ok();

    let patch = ctx
        .server
        .patch(&path)
        .authorization_bearer("w")
        .text("+1")
        .await;
    patch.assert_status_bad_request();
    patch.assert_json(&api_error_body_json(400, "bad_request"));
}

#[tokio::test]
async fn patch_requires_write_permission() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::restricted("r", "w")).await;
    let path = format!("/{bid}/k");

    let patch = ctx
        .server
        .patch(&path)
        .authorization_bearer("r")
        .text("+1")
        .await;
    patch.assert_status_forbidden();
    patch.assert_json(&api_error_body_json(403, "forbidden"));
}

#[tokio::test]
async fn patch_unauthorized_without_credential() {
    let ctx = TestContext::new();
    let bid = ctx
        .create_bucket(BucketSetup {
            read_key: Some("r".into()),
            write_key: Some("w".into()),
            ..BucketSetup::default()
        })
        .await;
    let path = format!("/{bid}/k");

    let patch = ctx.server.patch(&path).text("+1").await;
    patch.assert_status_unauthorized();
    patch.assert_json(&api_error_body_json(401, "unauthorized"));
}

#[tokio::test]
async fn patch_with_ttl_expires() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::restricted("r", "w")).await;
    let path = format!("/{bid}/ttl");

    ctx.server
        .patch(&format!("{path}?ttl=1"))
        .authorization_bearer("w")
        .text("+1")
        .await
        .assert_status_ok();

    sleep(Duration::from_millis(2100)).await;

    let get = ctx.server.get(&path).authorization_bearer("r").await;
    get.assert_status_not_found();
    get.assert_json(&api_error_body_json(404, "not_found"));
}

#[tokio::test]
async fn patch_scoped_token_prefix_enforced() {
    let ctx = TestContext::new();
    let bid = ctx
        .create_bucket(BucketSetup {
            secret_key: Some("sec".into()),
            signing_key: Some("sign".into()),
            ..BucketSetup::default()
        })
        .await;

    let form = TokenSetup {
        prefix: Some("x:".into()),
        permissions: "read,write,enumerate,delete".into(),
        ttl: 3600,
    };
    let tok = ctx.create_token(&bid, "sec", &form).await;
    tok.assert_status_ok();
    let access: serde_json::Value = tok.json();
    let token = access
        .get("access_token")
        .and_then(|v| v.as_str())
        .expect("access_token");

    let patch = ctx
        .server
        .patch(&format!("/{bid}/y:key"))
        .authorization_bearer(token)
        .text("+1")
        .await;
    patch.assert_status_forbidden();
    patch.assert_json(&api_error_body_json(403, "forbidden"));
}
