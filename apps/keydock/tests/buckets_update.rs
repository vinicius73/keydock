//! `PATCH /api/v1/{bucket}` policy updates.
//!
//! These tests pin the JSON PATCH contract:
//! - absent field  → no-op
//! - `null`        → clear (except `secret_key`, which must never be cleared)
//! - value         → set/rotate
//! - empty string  → `400 bad_request` (silent no-ops forbidden)
//!
//! Observability of the mutation goes through `GET /api/v1/{bucket}` (public
//! projection), so the assertions rely on the `has_*` flags and
//! `signing_key_generation` rather than any private field.

use axum::http::StatusCode;
use keydock_testkit::{BucketSetup, PolicyPatch, TestContext, api_error_body_json};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

async fn get_policy(ctx: &TestContext, bucket_id: &str, bearer: &str) -> Value {
    ctx.server
        .get(&format!("/api/v1/{bucket_id}"))
        .authorization_bearer(bearer)
        .await
        .json()
}

#[tokio::test]
async fn patch_clears_read_key_via_null() {
    let ctx = TestContext::new();
    let bid = ctx
        .create_bucket(BucketSetup {
            read_key: Some("r".into()),
            ..BucketSetup::admin("sec")
        })
        .await;

    ctx.patch_policy(&bid, "sec", &PolicyPatch::clear_read_key())
        .await
        .assert_status(StatusCode::NO_CONTENT);

    let body = get_policy(&ctx, &bid, "sec").await;
    assert_eq!(body.get("has_read_key"), Some(&json!(false)));
    assert_eq!(body.get("has_secret_key"), Some(&json!(true)));
}

#[tokio::test]
async fn patch_clears_write_key_via_null() {
    let ctx = TestContext::new();
    let bid = ctx
        .create_bucket(BucketSetup {
            write_key: Some("w".into()),
            ..BucketSetup::admin("sec")
        })
        .await;

    ctx.patch_policy(&bid, "sec", &PolicyPatch::clear_write_key())
        .await
        .assert_status(StatusCode::NO_CONTENT);

    let body = get_policy(&ctx, &bid, "sec").await;
    assert_eq!(body.get("has_write_key"), Some(&json!(false)));
}

#[tokio::test]
async fn patch_clears_signing_key_and_bumps_generation() {
    // Clearing `signing_key` is a material change and must bump the rotation
    // counter so existing tokens become invalid after rotation.
    let ctx = TestContext::new();
    let bid = ctx
        .create_bucket(BucketSetup {
            signing_key: Some("sign".into()),
            ..BucketSetup::admin("sec")
        })
        .await;

    let before = get_policy(&ctx, &bid, "sec").await;
    assert_eq!(before.get("has_signing_key"), Some(&json!(true)));
    assert_eq!(before.get("signing_key_generation"), Some(&json!(0)));

    ctx.patch_policy(&bid, "sec", &PolicyPatch::clear_signing_key())
        .await
        .assert_status(StatusCode::NO_CONTENT);

    let after = get_policy(&ctx, &bid, "sec").await;
    assert_eq!(after.get("has_signing_key"), Some(&json!(false)));
    assert_eq!(after.get("signing_key_generation"), Some(&json!(1)));
}

#[tokio::test]
async fn patch_rotate_signing_key_with_same_value_does_not_bump_generation() {
    // Idempotency guard: sending the same signing key twice must not churn
    // the generation counter, otherwise automated rotation scripts would
    // invalidate tokens on every deploy.
    let ctx = TestContext::new();
    let bid = ctx
        .create_bucket(BucketSetup {
            signing_key: Some("sign".into()),
            ..BucketSetup::admin("sec")
        })
        .await;

    ctx.patch_policy(&bid, "sec", &PolicyPatch::rotate_signing_key("sign"))
        .await
        .assert_status(StatusCode::NO_CONTENT);

    let body = get_policy(&ctx, &bid, "sec").await;
    assert_eq!(body.get("signing_key_generation"), Some(&json!(0)));
}

#[tokio::test]
async fn patch_rejects_clearing_secret_key() {
    // D8=A invariant: `secret_key` is the root credential; removing it would
    // orphan the bucket with no admin path back.
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::admin("sec")).await;

    let response = ctx
        .patch_policy(
            &bid,
            "sec",
            &PolicyPatch {
                secret_key: Some(serde_json::Value::Null),
                ..PolicyPatch::default()
            },
        )
        .await;
    response.assert_status_bad_request();
    response.assert_json(&api_error_body_json(400, "bad_request"));

    let body = get_policy(&ctx, &bid, "sec").await;
    assert_eq!(body.get("has_secret_key"), Some(&json!(true)));
}

#[tokio::test]
async fn patch_rotates_secret_key() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::admin("sec")).await;

    ctx.patch_policy(
        &bid,
        "sec",
        &PolicyPatch {
            secret_key: Some(json!("sec2")),
            ..PolicyPatch::default()
        },
    )
    .await
    .assert_status(StatusCode::NO_CONTENT);

    ctx.server
        .get(&format!("/api/v1/{bid}"))
        .authorization_bearer("sec")
        .await
        .assert_status_unauthorized();

    ctx.server
        .get(&format!("/api/v1/{bid}"))
        .authorization_bearer("sec2")
        .await
        .assert_status_ok();
}

#[tokio::test]
async fn patch_rejects_empty_string_on_read_key() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::admin("sec")).await;

    let response = ctx
        .patch_policy(
            &bid,
            "sec",
            &PolicyPatch {
                read_key: Some(json!("")),
                ..PolicyPatch::default()
            },
        )
        .await;
    response.assert_status_bad_request();
    response.assert_json(&api_error_body_json(400, "bad_request"));
}

#[tokio::test]
async fn patch_can_update_default_ttl_and_clear_it() {
    let ctx = TestContext::new();
    let bid = ctx
        .create_bucket(BucketSetup {
            default_ttl: Some(60),
            ..BucketSetup::admin("sec")
        })
        .await;

    ctx.patch_policy(
        &bid,
        "sec",
        &PolicyPatch {
            default_ttl: Some(json!(120)),
            ..PolicyPatch::default()
        },
    )
    .await
    .assert_status(StatusCode::NO_CONTENT);
    assert_eq!(
        get_policy(&ctx, &bid, "sec").await.get("default_ttl"),
        Some(&json!(120))
    );

    ctx.patch_policy(
        &bid,
        "sec",
        &PolicyPatch {
            default_ttl: Some(serde_json::Value::Null),
            ..PolicyPatch::default()
        },
    )
    .await
    .assert_status(StatusCode::NO_CONTENT);
    // `default_ttl` is omitted from the JSON when cleared (Option::is_none).
    let body = get_policy(&ctx, &bid, "sec").await;
    assert_eq!(body.get("default_ttl"), None);
}

#[tokio::test]
async fn patch_rejects_unknown_field() {
    // `deny_unknown_fields` guards against typos silently becoming no-ops.
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::admin("sec")).await;

    let response = ctx
        .server
        .patch(&format!("/api/v1/{bid}"))
        .authorization_bearer("sec")
        .json(&json!({ "unknown_field": "x" }))
        .await;
    response.assert_status_bad_request();
}

#[tokio::test]
async fn patch_empty_body_is_noop() {
    // A patch with no fields must be accepted as a well-formed no-op: this
    // is the natural contract of a JSON Merge-Patch and lets clients retry
    // idempotently without crafting conditional bodies.
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::admin("sec")).await;

    ctx.server
        .patch(&format!("/api/v1/{bid}"))
        .authorization_bearer("sec")
        .json(&json!({}))
        .await
        .assert_status(StatusCode::NO_CONTENT);
}
