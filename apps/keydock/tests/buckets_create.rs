//! Bucket creation flows (HTTP integration).

use keydock_testkit::{BucketSetup, TestContext};
use pretty_assertions::assert_eq;
use rstest::rstest;
use serde_json::json;
use uuid::Uuid;

#[rstest]
#[case("owner@example.com")]
#[case("other@example.net")]
#[tokio::test]
async fn create_public_bucket(#[case] email: &str) {
    let ctx = TestContext::new();
    let bid = ctx
        .create_bucket(BucketSetup {
            email: email.into(),
            ..BucketSetup::public()
        })
        .await;

    assert_eq!(bid.is_empty(), false);
    assert_eq!(Uuid::parse_str(&bid).is_ok(), true);
}

#[tokio::test]
async fn create_restricted_bucket() {
    let ctx = TestContext::new();
    let bid = ctx
        .create_bucket(BucketSetup {
            secret_key: Some("s".into()),
            read_key: Some("r".into()),
            write_key: Some("w".into()),
            signing_key: Some("sign".into()),
            ..BucketSetup::default()
        })
        .await;

    let response = ctx.server.get(&format!("/{bid}/k1")).await;
    response.assert_status_unauthorized();
    response.assert_json(&json!({
        "error": {
            "code": 401,
            "message": "unauthorized"
        }
    }));
}
