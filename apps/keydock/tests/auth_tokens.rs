//! Temporary token lifecycle and scope (HTTP integration).

use keydock_testkit::{BucketSetup, PolicyPatch, TestContext, TokenSetup};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

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

    let form = TokenSetup::read(3600);
    let forbidden = ctx.create_token(&bid, "w", &form).await;
    forbidden.assert_status_forbidden();
    forbidden.assert_json(&json!({
        "error": {
            "code": 403,
            "message": "forbidden"
        }
    }));

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

    let form = TokenSetup::read_prefixed("user:42:", 3600);
    let tok = ctx.create_token(&bid, "sec", &form).await;
    tok.assert_status_ok();
    let token: Value = tok.json();
    let access = access_token_str(&token).to_string();
    tok.assert_json(&json!({
        "access_token": access
    }));

    let ok = ctx
        .server
        .get(&format!("/{bid}/user:42:name"))
        .authorization_bearer(&access)
        .await;
    ok.assert_status_not_found();
    ok.assert_json(&json!({
        "error": {
            "code": 404,
            "message": "not_found"
        }
    }));
}

#[tokio::test]
async fn token_read_outside_prefix() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::signed("sec", "sign")).await;

    let form = TokenSetup::read_prefixed("user:42:", 3600);
    let tok = ctx.create_token(&bid, "sec", &form).await;
    tok.assert_status_ok();
    let token: Value = tok.json();
    let access = access_token_str(&token).to_string();
    tok.assert_json(&json!({
        "access_token": access
    }));

    let response = ctx
        .server
        .get(&format!("/{bid}/admin:config"))
        .authorization_bearer(&access)
        .await;
    response.assert_status_forbidden();
    response.assert_json(&json!({
        "error": {
            "code": 403,
            "message": "forbidden"
        }
    }));
}

#[tokio::test]
async fn token_expired() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::signed("sec", "sign")).await;

    let form = TokenSetup::expired();
    let tok = ctx.create_token(&bid, "sec", &form).await;
    tok.assert_status_ok();
    let token: Value = tok.json();
    let access = access_token_str(&token).to_string();
    tok.assert_json(&json!({
        "access_token": access
    }));

    let response = ctx
        .server
        .get(&format!("/{bid}/k1"))
        .authorization_bearer(&access)
        .await;
    response.assert_status_unauthorized();
    response.assert_json(&json!({
        "error": {
            "code": 401,
            "message": "unauthorized"
        }
    }));
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

    let form = TokenSetup::read(3600);
    let tok = ctx.create_token(&a, "sec", &form).await;
    tok.assert_status_ok();
    let token: Value = tok.json();
    let access = access_token_str(&token).to_string();
    tok.assert_json(&json!({
        "access_token": access
    }));

    let response = ctx
        .server
        .get(&format!("/{b}/k1"))
        .authorization_bearer(&access)
        .await;
    response.assert_status_unauthorized();
    response.assert_json(&json!({
        "error": {
            "code": 401,
            "message": "unauthorized"
        }
    }));
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

    let form = TokenSetup::read(3600);
    let tok = ctx.create_token(&bid, "sec", &form).await;
    tok.assert_status_ok();
    let token: Value = tok.json();
    let access = access_token_str(&token).to_string();
    tok.assert_json(&json!({
        "access_token": access
    }));

    let ok_before = ctx
        .server
        .get(&format!("/{bid}/k1"))
        .authorization_bearer(&access)
        .await;
    ok_before.assert_status_not_found();
    ok_before.assert_json(&json!({
        "error": {
            "code": 404,
            "message": "not_found"
        }
    }));

    let patch = ctx
        .patch_policy(&bid, "sec", &PolicyPatch::rotate_signing_key("sign2"))
        .await;
    patch.assert_status_no_content();

    let unauthorized = ctx
        .server
        .get(&format!("/{bid}/k1"))
        .authorization_bearer(&access)
        .await;
    unauthorized.assert_status_unauthorized();
    unauthorized.assert_json(&json!({
        "error": {
            "code": 401,
            "message": "unauthorized"
        }
    }));

    let tok2 = ctx.create_token(&bid, "sec", &form).await;
    tok2.assert_status_ok();
    let token2: Value = tok2.json();
    let access2 = access_token_str(&token2).to_string();
    tok2.assert_json(&json!({
        "access_token": access2
    }));

    let ok_after = ctx
        .server
        .get(&format!("/{bid}/k1"))
        .authorization_bearer(&access2)
        .await;
    ok_after.assert_status_not_found();
    ok_after.assert_json(&json!({
        "error": {
            "code": 404,
            "message": "not_found"
        }
    }));
}
