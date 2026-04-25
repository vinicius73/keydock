//! Compatibility test for `DELETE /{bucket}/` (trailing slash alias).
//!
//! The trailing-slash form shares the `delete_bucket` handler with `DELETE /{bucket}`,
//! so these cases pin down:
//!
//! - Successful delete (204) with a valid `secret_key`.
//! - Envelope-shaped 403 for anonymous callers (delete is admin-only, so
//!   `require_admin` returns forbidden, mirroring `DELETE /{bucket}`).
//! - Envelope-shaped 404 for unknown buckets.
//! - Both `DELETE /{bucket}` and `DELETE /{bucket}/` accept the same credential.
//!
//! Anchoring the trailing-slash semantics here prevents a silent regression if the
//! alias is ever dropped from the router.

use keydock_testkit::{BucketSetup, TestContext, api_error_body_json, basic_auth_header};
use pretty_assertions::assert_eq;
use rstest::rstest;
use uuid::Uuid;

#[tokio::test]
async fn delete_bucket_trailing_slash_as_admin_returns_204() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::admin("sec")).await;

    let response = ctx
        .server
        .delete(&format!("/{bid}/"))
        .authorization_bearer("sec")
        .await;
    response.assert_status(axum::http::StatusCode::NO_CONTENT);
    assert_eq!(response.as_bytes().is_empty(), true);

    let followup = ctx
        .server
        .get(&format!("/{bid}/"))
        .authorization_bearer("sec")
        .await;
    followup.assert_status_not_found();
    followup.assert_json(&api_error_body_json(404, "not_found"));
}

#[tokio::test]
async fn delete_bucket_trailing_slash_anonymous_returns_403() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::admin("sec")).await;

    let response = ctx.server.delete(&format!("/{bid}/")).await;
    response.assert_status_forbidden();
    response.assert_json(&api_error_body_json(403, "forbidden"));
}

#[tokio::test]
async fn delete_bucket_trailing_slash_unknown_returns_404() {
    let ctx = TestContext::new();
    let unknown = Uuid::new_v4().to_string();

    let response = ctx
        .server
        .delete(&format!("/{unknown}/"))
        .add_header(
            axum::http::header::AUTHORIZATION,
            basic_auth_header("anything"),
        )
        .await;
    response.assert_status_not_found();
    response.assert_json(&api_error_body_json(404, "not_found"));
}

#[rstest]
#[case::no_slash("")]
#[case::with_slash("/")]
#[tokio::test]
async fn delete_bucket_both_forms_accept_admin_credential(#[case] suffix: &str) {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::admin("sec")).await;

    let path = format!("/{bid}{suffix}");
    let response = ctx.server.delete(&path).authorization_bearer("sec").await;
    response.assert_status(axum::http::StatusCode::NO_CONTENT);
}
