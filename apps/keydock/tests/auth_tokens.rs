//! Temporary token lifecycle and scope (HTTP integration).

mod common;

use pretty_assertions::assert_eq;
use serde_json::Value;

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
    let body: Value = forbidden.json();
    assert_eq!(body["error"], "forbidden");

    let ok = create_token(&server, &bid, "sec", &form).await;
    ok.assert_status_ok();
    let body: Value = ok.json();
    assert!(access_token_str(&body).contains('.'));
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
    let access = access_token_str(&token);

    let ok = server
        .get(&format!("/{bid}/user:42:name"))
        .authorization_bearer(access)
        .await;
    ok.assert_status_ok();
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
    let access = access_token_str(&token);

    let response = server
        .get(&format!("/{bid}/admin:config"))
        .authorization_bearer(access)
        .await;
    response.assert_status_forbidden();
    let body: Value = response.json();
    assert_eq!(body["error"], "forbidden");
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
    let access = access_token_str(&token);

    let response = server
        .get(&format!("/{bid}/k1"))
        .authorization_bearer(access)
        .await;
    response.assert_status_unauthorized();
    let body: Value = response.json();
    assert_eq!(body["error"], "unauthorized");
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
    let access = access_token_str(&token);

    let response = server
        .get(&format!("/{b}/k1"))
        .authorization_bearer(access)
        .await;
    response.assert_status_unauthorized();
    let body: Value = response.json();
    assert_eq!(body["error"], "unauthorized");
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
    let access = access_token_str(&token);

    let ok_before = server
        .get(&format!("/{bid}/k1"))
        .authorization_bearer(access)
        .await;
    ok_before.assert_status_ok();

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
        .authorization_bearer(access)
        .await;
    unauthorized.assert_status_unauthorized();

    let tok2 = create_token(&server, &bid, "sec", &form).await;
    tok2.assert_status_ok();
    let token2: Value = tok2.json();
    let access2 = access_token_str(&token2);

    let ok_after = server
        .get(&format!("/{bid}/k1"))
        .authorization_bearer(access2)
        .await;
    ok_after.assert_status_ok();
}
