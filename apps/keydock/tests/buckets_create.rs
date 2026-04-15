//! Bucket creation flows (HTTP integration).

mod common;

use pretty_assertions::assert_eq;
use rstest::rstest;
use serde_json::Value;
use uuid::Uuid;

use common::buckets::{CreateBucketForm, create_bucket};

#[rstest]
#[case("owner@example.com")]
#[case("other@example.net")]
#[tokio::test]
async fn create_public_bucket(#[case] email: &str) {
    let (_dir, server) = keydock_testkit::test_app().expect("test_app");
    let bid = create_bucket(
        &server,
        &CreateBucketForm {
            email: email.into(),
            secret_key: None,
            read_key: None,
            write_key: None,
            signing_key: None,
            default_ttl: None,
        },
    )
    .await;

    assert!(!bid.is_empty());
    assert!(Uuid::parse_str(&bid).is_ok());
}

#[tokio::test]
async fn create_restricted_bucket() {
    let (_dir, server) = keydock_testkit::test_app().expect("test_app");
    let bid = create_bucket(
        &server,
        &CreateBucketForm {
            email: "o@example.com".into(),
            secret_key: Some("s".into()),
            read_key: Some("r".into()),
            write_key: Some("w".into()),
            signing_key: Some("sign".into()),
            default_ttl: None,
        },
    )
    .await;

    let response = server.get(&format!("/{bid}/k1")).await;
    response.assert_status_unauthorized();
    let body: Value = response.json();
    assert_eq!(body["error"], "unauthorized");
}
