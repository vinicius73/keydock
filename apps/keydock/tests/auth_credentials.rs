//! Credential channel and permission matrix (HTTP integration).

mod common;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STD;
use common::buckets::{CreateBucketForm, create_bucket};
use pretty_assertions::assert_eq;
use serde_json::Value;

#[tokio::test]
async fn bearer_secret_key_grants_admin() {
    let (_dir, server) = keydock_testkit::test_app();
    let bid = create_bucket(
        &server,
        &CreateBucketForm {
            email: "o@example.com".into(),
            secret_key: Some("sec".into()),
            read_key: None,
            write_key: None,
            signing_key: None,
            default_ttl: None,
        },
    )
    .await;

    let response = server
        .put(&format!("/{bid}/k1"))
        .authorization_bearer("sec")
        .await;
    response.assert_status_ok();
    let body: Value = response.json();
    assert_eq!(body["ok"], true);
}

#[tokio::test]
async fn bearer_write_key_grants_write() {
    let (_dir, server) = keydock_testkit::test_app();
    let bid = create_bucket(
        &server,
        &CreateBucketForm {
            email: "o@example.com".into(),
            secret_key: None,
            read_key: Some("r".into()),
            write_key: Some("w".into()),
            signing_key: None,
            default_ttl: None,
        },
    )
    .await;

    let ok = server
        .put(&format!("/{bid}/k1"))
        .authorization_bearer("w")
        .await;
    ok.assert_status_ok();

    let forbidden = server
        .put(&format!("/{bid}/k1"))
        .authorization_bearer("r")
        .await;
    forbidden.assert_status_forbidden();
    let body: Value = forbidden.json();
    assert_eq!(body["error"], "forbidden");
}

#[tokio::test]
async fn bearer_read_key_grants_read() {
    let (_dir, server) = keydock_testkit::test_app();
    let bid = create_bucket(
        &server,
        &CreateBucketForm {
            email: "o@example.com".into(),
            secret_key: None,
            read_key: Some("r".into()),
            write_key: None,
            signing_key: None,
            default_ttl: None,
        },
    )
    .await;

    let ok = server
        .get(&format!("/{bid}/k1"))
        .authorization_bearer("r")
        .await;
    ok.assert_status_ok();

    let unauthorized = server.get(&format!("/{bid}/k1")).await;
    unauthorized.assert_status_unauthorized();
    let body: Value = unauthorized.json();
    assert_eq!(body["error"], "unauthorized");
}

#[tokio::test]
async fn query_param_access_token_works() {
    let (_dir, server) = keydock_testkit::test_app();
    let bid = create_bucket(
        &server,
        &CreateBucketForm {
            email: "o@example.com".into(),
            secret_key: None,
            read_key: Some("r".into()),
            write_key: None,
            signing_key: None,
            default_ttl: None,
        },
    )
    .await;

    let response = server.get(&format!("/{bid}/k1?access_token=r")).await;
    response.assert_status_ok();
}

#[tokio::test]
async fn query_param_key_works() {
    let (_dir, server) = keydock_testkit::test_app();
    let bid = create_bucket(
        &server,
        &CreateBucketForm {
            email: "o@example.com".into(),
            secret_key: None,
            read_key: Some("r".into()),
            write_key: None,
            signing_key: None,
            default_ttl: None,
        },
    )
    .await;

    let response = server.get(&format!("/{bid}/k1?key=r")).await;
    response.assert_status_ok();
}

#[tokio::test]
async fn basic_auth_works() {
    let (_dir, server) = keydock_testkit::test_app();
    let bid = create_bucket(
        &server,
        &CreateBucketForm {
            email: "o@example.com".into(),
            secret_key: None,
            read_key: Some("r".into()),
            write_key: None,
            signing_key: None,
            default_ttl: None,
        },
    )
    .await;

    let encoded = BASE64_STD.encode(b"r:ignored");
    let response = server
        .get(&format!("/{bid}/k1"))
        .authorization(format!("Basic {encoded}"))
        .await;
    response.assert_status_ok();
}

#[tokio::test]
async fn wrong_credential_returns_401() {
    let (_dir, server) = keydock_testkit::test_app();
    let bid = create_bucket(
        &server,
        &CreateBucketForm {
            email: "o@example.com".into(),
            secret_key: Some("sec".into()),
            read_key: None,
            write_key: None,
            signing_key: None,
            default_ttl: None,
        },
    )
    .await;

    let response = server
        .get(&format!("/{bid}/k1"))
        .authorization_bearer("wrong")
        .await;
    response.assert_status_unauthorized();
    let body: Value = response.json();
    assert_eq!(body["error"], "unauthorized");
}

#[tokio::test]
async fn missing_bucket_returns_404() {
    let (_dir, server) = keydock_testkit::test_app();
    let response = server.get("/no-such-bucket/k").await;
    response.assert_status_not_found();
    let body: Value = response.json();
    assert_eq!(body["error"], "not_found");
}

#[tokio::test]
async fn anonymous_public_bucket_read() {
    let (_dir, server) = keydock_testkit::test_app();
    let bid = create_bucket(
        &server,
        &CreateBucketForm {
            email: "o@example.com".into(),
            secret_key: None,
            read_key: None,
            write_key: None,
            signing_key: None,
            default_ttl: None,
        },
    )
    .await;

    let response = server.get(&format!("/{bid}/k1")).await;
    response.assert_status_ok();
}

#[tokio::test]
async fn anonymous_restricted_bucket_read() {
    let (_dir, server) = keydock_testkit::test_app();
    let bid = create_bucket(
        &server,
        &CreateBucketForm {
            email: "o@example.com".into(),
            secret_key: None,
            read_key: Some("r".into()),
            write_key: None,
            signing_key: None,
            default_ttl: None,
        },
    )
    .await;

    let response = server.get(&format!("/{bid}/k1")).await;
    response.assert_status_unauthorized();
    let body: Value = response.json();
    assert_eq!(body["error"], "unauthorized");
}
