//! Bucket creation flows (HTTP integration).

use bytes::Bytes;
use keydock_testkit::{BucketSetup, TestContext, api_error_body_json};
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

#[rstest]
#[case::bare_word("x")]
#[case::at_only("@")]
#[case::no_domain_dot("a@b")]
#[case::empty_local("@b.c")]
#[case::leading_dot_domain("a@.b")]
#[case::trailing_dot_domain("a@b.")]
#[case::multiple_at("a@b@c.d")]
#[case::blank("   ")]
#[tokio::test]
async fn create_bucket_rejects_structurally_invalid_email(#[case] email: &str) {
    // Minimal structural validation catches the obviously
    // broken cases without pulling in a full RFC 5322 parser. Each input
    // here is shaped to violate a different clause of the rule so we have
    // coverage over every rejection branch.
    let ctx = TestContext::new();
    let payload = BucketSetup {
        email: email.into(),
        ..BucketSetup::public()
    };
    let body = serde_urlencoded::to_string(&payload).expect("encode form");

    let response = ctx
        .server
        .post("/")
        .bytes(Bytes::from(body))
        .content_type("application/x-www-form-urlencoded")
        .await;
    response.assert_status_bad_request();
    response.assert_json(&api_error_body_json(400, "bad_request"));
}

#[tokio::test]
async fn create_bucket_accepts_minimally_valid_email_with_subdomain() {
    // Ensure the validator does not over-reject common shapes like
    // subdomain-heavy emails and plus-addressed local parts.
    let ctx = TestContext::new();
    for email in ["user+tag@sub.example.com", "a.b@c.d"] {
        let bid = ctx
            .create_bucket(BucketSetup {
                email: email.into(),
                ..BucketSetup::public()
            })
            .await;
        assert_eq!(Uuid::parse_str(&bid).is_ok(), true, "email={email}");
    }
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
