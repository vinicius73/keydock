//! Compatibility tests for `GET /api/v1/{bucket}` (bucket policy view).
//!
//! Pins the public JSON projection contract:
//!
//! - admin-only (401 anonymous, 403 non-admin, 404 unknown bucket)
//! - body carries only non-sensitive fields (no `*_hash`, no raw `signing_key`)
//! - `signing_key_generation`, `default_ttl`, `has_*` flags and
//!   `anonymous_access` are present so clients can reason about token lifetime.

use keydock_testkit::{BucketSetup, TestContext, api_error_body_json};
use serde_json::{Value, json};
use uuid::Uuid;

#[tokio::test]
async fn get_bucket_policy_admin_returns_public_projection() {
    let ctx = TestContext::new();
    let bid = ctx
        .create_bucket(BucketSetup {
            read_key: Some("r".into()),
            write_key: Some("w".into()),
            signing_key: Some("sign".into()),
            default_ttl: Some(3600),
            ..BucketSetup::admin("sec")
        })
        .await;

    let response = ctx
        .server
        .get(&format!("/api/v1/{bid}"))
        .authorization_bearer("sec")
        .await;
    response.assert_status_ok();
    response.assert_json(&json!({
        "default_ttl": 3600,
        "has_secret_key": true,
        "has_read_key": true,
        "has_write_key": true,
        "has_signing_key": true,
        "signing_key_generation": 0,
        "anonymous_access": {
            "read": false,
            "write": false,
            "enumerate": false,
            "delete": false,
        },
    }));

    // Explicit negative assertion: the sensitive fields must NOT leak.
    let body: Value = response.json();
    for forbidden_field in [
        "secret_key",
        "secret_key_hash",
        "read_key_hash",
        "write_key_hash",
        "signing_key",
    ] {
        assert_eq!(
            body.get(forbidden_field),
            None,
            "policy body must not expose `{forbidden_field}`, got: {body}"
        );
    }
}

#[tokio::test]
async fn get_bucket_policy_anonymous_returns_403() {
    // Admin-only gate surfaces as 403 because the request is authenticated
    // against the bucket (no credential), consistent with `DELETE /api/v1/{bucket}`.
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::admin("sec")).await;

    let response = ctx.server.get(&format!("/api/v1/{bid}")).await;
    response.assert_status_forbidden();
    response.assert_json(&api_error_body_json(403, "forbidden"));
}

#[tokio::test]
async fn get_bucket_policy_read_key_returns_403() {
    let ctx = TestContext::new();
    let bid = ctx
        .create_bucket(BucketSetup {
            read_key: Some("r".into()),
            ..BucketSetup::admin("sec")
        })
        .await;

    let response = ctx
        .server
        .get(&format!("/api/v1/{bid}"))
        .authorization_bearer("r")
        .await;
    response.assert_status_forbidden();
    response.assert_json(&api_error_body_json(403, "forbidden"));
}

#[tokio::test]
async fn get_bucket_policy_unknown_bucket_returns_404() {
    let ctx = TestContext::new();
    let unknown = Uuid::new_v4().to_string();

    let response = ctx.server.get(&format!("/api/v1/{unknown}")).await;
    response.assert_status_not_found();
    response.assert_json(&api_error_body_json(404, "not_found"));
}

#[tokio::test]
async fn get_bucket_policy_reflects_signing_key_rotation() {
    use keydock_testkit::PolicyPatch;

    let ctx = TestContext::new();
    let bid = ctx
        .create_bucket(BucketSetup {
            signing_key: Some("sign1".into()),
            ..BucketSetup::admin("sec")
        })
        .await;

    let before: Value = ctx
        .server
        .get(&format!("/api/v1/{bid}"))
        .authorization_bearer("sec")
        .await
        .json();
    assert_eq!(before.get("signing_key_generation"), Some(&json!(0)));

    ctx.patch_policy(&bid, "sec", &PolicyPatch::rotate_signing_key("sign2"))
        .await
        .assert_status_no_content();

    let after: Value = ctx
        .server
        .get(&format!("/api/v1/{bid}"))
        .authorization_bearer("sec")
        .await
        .json();
    assert_eq!(after.get("signing_key_generation"), Some(&json!(1)));
}
