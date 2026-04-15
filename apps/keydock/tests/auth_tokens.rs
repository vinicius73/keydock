//! Temporary token lifecycle and scope (HTTP integration).

mod common;

use serde_json::{Value, json};

use common::buckets::{CreateBucketForm, create_bucket};
use common::tokens::{CreateTokenForm, UpdatePolicyForm, create_token, patch_policy};

#[track_caller]
fn access_token_str(body: &Value) -> &str {
    body.get("access_token")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("expected string access_token in {body:?}"))
}

#[tokio::test]
async fn create_token_requires_admin() {
    let (_dir, server) = keydock_testkit::test_app().expect("test_app");
    let bid = create_bucket(
        &server,
        &CreateBucketForm {
            email: "o@example.com".into(),
            secret_key: Some("sec".into()),
            read_key: None,
            write_key: Some("w".into()),
            signing_key: Some("sign".into()),
            default_ttl: None,
        },
    )
    .await;

    let form = CreateTokenForm {
        prefix: None,
        permissions: "read".into(),
        ttl: 3600,
    };
    let forbidden = create_token(&server, &bid, "w", &form).await;
    forbidden.assert_status_forbidden();
    forbidden.assert_json(&json!({
        "error": "forbidden"
    }));

    let ok = create_token(&server, &bid, "sec", &form).await;
    ok.assert_status_ok();
    let body: Value = ok.json();
    let access = access_token_str(&body).to_string();
    assert!(access.contains('.'));
    ok.assert_json(&json!({
        "access_token": access
    }));
}

#[tokio::test]
async fn token_read_within_prefix() {
    let (_dir, server) = keydock_testkit::test_app().expect("test_app");
    let bid = create_bucket(
        &server,
        &CreateBucketForm {
            email: "o@example.com".into(),
            secret_key: Some("sec".into()),
            read_key: None,
            write_key: None,
            signing_key: Some("sign".into()),
            default_ttl: None,
        },
    )
    .await;

    let form = CreateTokenForm {
        prefix: Some("user:42:".into()),
        permissions: "read".into(),
        ttl: 3600,
    };
    let tok = create_token(&server, &bid, "sec", &form).await;
    tok.assert_status_ok();
    let token: Value = tok.json();
    let access = access_token_str(&token).to_string();
    tok.assert_json(&json!({
        "access_token": access
    }));

    let ok = server
        .get(&format!("/{bid}/user:42:name"))
        .authorization_bearer(&access)
        .await;
    ok.assert_status_ok();
    ok.assert_json(&json!({
        "ok": true
    }));
}

#[tokio::test]
async fn token_read_outside_prefix() {
    let (_dir, server) = keydock_testkit::test_app().expect("test_app");
    let bid = create_bucket(
        &server,
        &CreateBucketForm {
            email: "o@example.com".into(),
            secret_key: Some("sec".into()),
            read_key: None,
            write_key: None,
            signing_key: Some("sign".into()),
            default_ttl: None,
        },
    )
    .await;

    let form = CreateTokenForm {
        prefix: Some("user:42:".into()),
        permissions: "read".into(),
        ttl: 3600,
    };
    let tok = create_token(&server, &bid, "sec", &form).await;
    tok.assert_status_ok();
    let token: Value = tok.json();
    let access = access_token_str(&token).to_string();
    tok.assert_json(&json!({
        "access_token": access
    }));

    let response = server
        .get(&format!("/{bid}/admin:config"))
        .authorization_bearer(&access)
        .await;
    response.assert_status_forbidden();
    response.assert_json(&json!({
        "error": "forbidden"
    }));
}

#[tokio::test]
async fn token_expired() {
    let (_dir, server) = keydock_testkit::test_app().expect("test_app");
    let bid = create_bucket(
        &server,
        &CreateBucketForm {
            email: "o@example.com".into(),
            secret_key: Some("sec".into()),
            read_key: None,
            write_key: None,
            signing_key: Some("sign".into()),
            default_ttl: None,
        },
    )
    .await;

    let form = CreateTokenForm {
        prefix: None,
        permissions: "read".into(),
        ttl: 0,
    };
    let tok = create_token(&server, &bid, "sec", &form).await;
    tok.assert_status_ok();
    let token: Value = tok.json();
    let access = access_token_str(&token).to_string();
    tok.assert_json(&json!({
        "access_token": access
    }));

    let response = server
        .get(&format!("/{bid}/k1"))
        .authorization_bearer(&access)
        .await;
    response.assert_status_unauthorized();
    response.assert_json(&json!({
        "error": "unauthorized"
    }));
}

#[tokio::test]
async fn token_wrong_bucket() {
    let (_dir, server) = keydock_testkit::test_app().expect("test_app");
    let a = create_bucket(
        &server,
        &CreateBucketForm {
            email: "a@example.com".into(),
            secret_key: Some("sec".into()),
            read_key: None,
            write_key: None,
            signing_key: Some("sign".into()),
            default_ttl: None,
        },
    )
    .await;
    let b = create_bucket(
        &server,
        &CreateBucketForm {
            email: "b@example.com".into(),
            secret_key: Some("sec".into()),
            read_key: None,
            write_key: None,
            signing_key: Some("sign".into()),
            default_ttl: None,
        },
    )
    .await;

    let form = CreateTokenForm {
        prefix: None,
        permissions: "read".into(),
        ttl: 3600,
    };
    let tok = create_token(&server, &a, "sec", &form).await;
    tok.assert_status_ok();
    let token: Value = tok.json();
    let access = access_token_str(&token).to_string();
    tok.assert_json(&json!({
        "access_token": access
    }));

    let response = server
        .get(&format!("/{b}/k1"))
        .authorization_bearer(&access)
        .await;
    response.assert_status_unauthorized();
    response.assert_json(&json!({
        "error": "unauthorized"
    }));
}

#[tokio::test]
async fn token_invalidated_after_signing_key_rotation() {
    let (_dir, server) = keydock_testkit::test_app().expect("test_app");
    let bid = create_bucket(
        &server,
        &CreateBucketForm {
            email: "o@example.com".into(),
            secret_key: Some("sec".into()),
            read_key: None,
            write_key: None,
            signing_key: Some("sign1".into()),
            default_ttl: None,
        },
    )
    .await;

    let form = CreateTokenForm {
        prefix: None,
        permissions: "read".into(),
        ttl: 3600,
    };
    let tok = create_token(&server, &bid, "sec", &form).await;
    tok.assert_status_ok();
    let token: Value = tok.json();
    let access = access_token_str(&token).to_string();
    tok.assert_json(&json!({
        "access_token": access
    }));

    let ok_before = server
        .get(&format!("/{bid}/k1"))
        .authorization_bearer(&access)
        .await;
    ok_before.assert_status_ok();
    ok_before.assert_json(&json!({
        "ok": true
    }));

    let patch = patch_policy(
        &server,
        &bid,
        "sec",
        &UpdatePolicyForm {
            signing_key: Some("sign2".into()),
        },
    )
    .await;
    patch.assert_status_no_content();

    let unauthorized = server
        .get(&format!("/{bid}/k1"))
        .authorization_bearer(&access)
        .await;
    unauthorized.assert_status_unauthorized();
    unauthorized.assert_json(&json!({
        "error": "unauthorized"
    }));

    let tok2 = create_token(&server, &bid, "sec", &form).await;
    tok2.assert_status_ok();
    let token2: Value = tok2.json();
    let access2 = access_token_str(&token2).to_string();
    tok2.assert_json(&json!({
        "access_token": access2
    }));

    let ok_after = server
        .get(&format!("/{bid}/k1"))
        .authorization_bearer(&access2)
        .await;
    ok_after.assert_status_ok();
    ok_after.assert_json(&json!({
        "ok": true
    }));
}
