//! Temporary token lifecycle and scope (HTTP integration).

use pretty_assertions::assert_eq;
use serde_json::{Value, json};

use keydock_testkit::{BucketSetup, PolicyPatch, TestContext, TokenSetup, api_error_body_json};

#[track_caller]
fn access_token_str(body: &Value) -> &str {
    body.get("access_token")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("expected string access_token in {body:?}"))
}

#[tokio::test]
async fn create_token_requires_admin() {
    let ctx = TestContext::new();
    let bid = ctx
        .create_bucket(BucketSetup {
            write_key: Some("w".into()),
            signing_key: Some("sign".into()),
            ..BucketSetup::admin("sec")
        })
        .await;

    let form = TokenSetup::read("scope:", 3600);
    let forbidden = ctx.create_token(&bid, "w", &form).await;
    forbidden.assert_status_forbidden();
    forbidden.assert_json(&api_error_body_json(403, "forbidden"));

    let ok = ctx.create_token(&bid, "sec", &form).await;
    ok.assert_status_ok();
    let body: Value = ok.json();
    let access = access_token_str(&body).to_string();
    assert_eq!(access.contains('.'), true);
    ok.assert_json(&json!({
        "access_token": access
    }));
}

#[tokio::test]
async fn token_read_within_prefix() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::signed("sec", "sign")).await;

    let form = TokenSetup::read("user:42:", 3600);
    let tok = ctx.create_token(&bid, "sec", &form).await;
    tok.assert_status_ok();
    let token: Value = tok.json();
    let access = access_token_str(&token).to_string();
    tok.assert_json(&json!({
        "access_token": access
    }));

    let ok = ctx
        .server
        .get(&format!("/api/v1/{bid}/user:42:name"))
        .authorization_bearer(&access)
        .await;
    ok.assert_status_not_found();
    ok.assert_json(&api_error_body_json(404, "not_found"));
}

#[tokio::test]
async fn token_read_outside_prefix() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::signed("sec", "sign")).await;

    let form = TokenSetup::read("user:42:", 3600);
    let tok = ctx.create_token(&bid, "sec", &form).await;
    tok.assert_status_ok();
    let token: Value = tok.json();
    let access = access_token_str(&token).to_string();
    tok.assert_json(&json!({
        "access_token": access
    }));

    let response = ctx
        .server
        .get(&format!("/api/v1/{bid}/admin:config"))
        .authorization_bearer(&access)
        .await;
    response.assert_status_forbidden();
    response.assert_json(&api_error_body_json(403, "forbidden"));
}

#[tokio::test]
async fn token_expired_is_rejected_on_use() {
    // `POST /tokens/` now rejects `ttl <= 0`, so we mint the expired token
    // directly through the testkit helper (which uses the same signing path as
    // the server) and assert that `verify` refuses it at the first request.
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::signed("sec", "sign")).await;

    let access = ctx.mint_expired_read_token(&bid, "sign");

    let response = ctx
        .server
        .get(&format!("/api/v1/{bid}/k1"))
        .authorization_bearer(&access)
        .await;
    response.assert_status_unauthorized();
    response.assert_json(&api_error_body_json(401, "unauthorized"));
}

#[tokio::test]
async fn token_mint_rejects_non_positive_ttl() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::signed("sec", "sign")).await;

    for ttl in [0_i64, -1, i64::MIN] {
        let form = TokenSetup::read("scope:", ttl);
        let response = ctx.create_token(&bid, "sec", &form).await;
        response.assert_status_bad_request();
        response.assert_json(&api_error_body_json(400, "bad_request"));
    }
}

#[tokio::test]
async fn token_mint_rejects_empty_prefix() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::signed("sec", "sign")).await;

    let form = TokenSetup::read("", 3600);
    let response = ctx.create_token(&bid, "sec", &form).await;
    response.assert_status_bad_request();
    response.assert_json(&api_error_body_json(400, "bad_request"));
}

#[tokio::test]
async fn token_wrong_bucket() {
    let ctx = TestContext::new();
    let a = ctx
        .create_bucket(BucketSetup {
            email: "a@example.com".into(),
            ..BucketSetup::signed("sec", "sign")
        })
        .await;
    let b = ctx
        .create_bucket(BucketSetup {
            email: "b@example.com".into(),
            ..BucketSetup::signed("sec", "sign")
        })
        .await;

    let form = TokenSetup::read("scope:", 3600);
    let tok = ctx.create_token(&a, "sec", &form).await;
    tok.assert_status_ok();
    let token: Value = tok.json();
    let access = access_token_str(&token).to_string();
    tok.assert_json(&json!({
        "access_token": access
    }));

    let response = ctx
        .server
        .get(&format!("/api/v1/{b}/k1"))
        .authorization_bearer(&access)
        .await;
    response.assert_status_unauthorized();
    response.assert_json(&api_error_body_json(401, "unauthorized"));
}

#[tokio::test]
async fn token_invalidated_after_signing_key_rotation() {
    let ctx = TestContext::new();
    let bid = ctx
        .create_bucket(BucketSetup {
            signing_key: Some("sign1".into()),
            ..BucketSetup::admin("sec")
        })
        .await;

    let form = TokenSetup::read("scope:", 3600);
    let tok = ctx.create_token(&bid, "sec", &form).await;
    tok.assert_status_ok();
    let token: Value = tok.json();
    let access = access_token_str(&token).to_string();
    tok.assert_json(&json!({
        "access_token": access
    }));

    // Read a key *inside* the token scope so the signature path (not the
    // prefix guard) is the one exercised across rotation.
    let probe_key = format!("/api/v1/{bid}/scope:k1");

    let ok_before = ctx
        .server
        .get(&probe_key)
        .authorization_bearer(&access)
        .await;
    ok_before.assert_status_not_found();
    ok_before.assert_json(&api_error_body_json(404, "not_found"));

    let patch = ctx
        .patch_policy(&bid, "sec", &PolicyPatch::rotate_signing_key("sign2"))
        .await;
    patch.assert_status_no_content();

    let unauthorized = ctx
        .server
        .get(&probe_key)
        .authorization_bearer(&access)
        .await;
    unauthorized.assert_status_unauthorized();
    unauthorized.assert_json(&api_error_body_json(401, "unauthorized"));

    let tok2 = ctx.create_token(&bid, "sec", &form).await;
    tok2.assert_status_ok();
    let token2: Value = tok2.json();
    let access2 = access_token_str(&token2).to_string();
    tok2.assert_json(&json!({
        "access_token": access2
    }));

    let ok_after = ctx
        .server
        .get(&probe_key)
        .authorization_bearer(&access2)
        .await;
    ok_after.assert_status_not_found();
    ok_after.assert_json(&api_error_body_json(404, "not_found"));
}
