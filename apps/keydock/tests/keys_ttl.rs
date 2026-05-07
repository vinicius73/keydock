//! TTL behavior (HTTP integration).

use std::time::Duration;

use axum::http::header;
use keydock_testkit::{BucketSetup, TestContext};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use tokio::time::sleep;

#[tokio::test]
async fn get_returns_404_after_ttl_expires() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::public()).await;
    let path = format!("/api/v1/{bid}/ttl1");

    ctx.server
        .post(&path)
        .add_query_param("ttl", "1")
        .text("v")
        .await
        .assert_status_ok();

    sleep(Duration::from_secs(2)).await;

    let get = ctx.server.get(&path).await;
    get.assert_status_not_found();
}

#[tokio::test]
async fn write_renews_ttl() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::public()).await;
    let path = format!("/api/v1/{bid}/ttl-renew");

    ctx.server
        .post(&path)
        .add_query_param("ttl", "3")
        .text("a")
        .await
        .assert_status_ok();

    sleep(Duration::from_secs(1)).await;

    ctx.server
        .post(&path)
        .add_query_param("ttl", "3")
        .text("b")
        .await
        .assert_status_ok();

    sleep(Duration::from_secs(2)).await;

    let get = ctx.server.get(&path).await;
    get.assert_status_ok();
    get.assert_text("b");
}

#[tokio::test]
async fn create_bucket_without_default_ttl_uses_hosted_default_of_seven_days() {
    // Omitting `default_ttl` on create must fall back to
    // the hosted-compatible default of 7 days (604800 seconds). The public
    // `GET /api/v1/{bucket}` projection is the authoritative surface that clients
    // use to discover this value, so we assert it there end-to-end.
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::admin("sec")).await;

    let response = ctx
        .server
        .get(&format!("/api/v1/{bid}"))
        .authorization_bearer("sec")
        .await;
    response.assert_status_ok();
    let body: Value = response.json();
    assert_eq!(body.get("default_ttl"), Some(&json!(604_800)));
}

#[tokio::test]
async fn create_bucket_with_explicit_default_ttl_zero_preserves_no_expiry() {
    // Explicit `0` must round-trip unchanged: the `resolve_ttl` contract
    // treats `Some(0)` as "no expiration", and clients rely on that signal
    // to opt out of the hosted default.
    let ctx = TestContext::new();
    let bid = ctx
        .create_bucket(BucketSetup {
            default_ttl: Some(0),
            ..BucketSetup::admin("sec")
        })
        .await;

    let response = ctx
        .server
        .get(&format!("/api/v1/{bid}"))
        .authorization_bearer("sec")
        .await;
    response.assert_status_ok();
    let body: Value = response.json();
    assert_eq!(body.get("default_ttl"), Some(&json!(0)));
}

#[tokio::test]
async fn create_bucket_with_explicit_default_ttl_is_preserved() {
    let ctx = TestContext::new();
    let bid = ctx
        .create_bucket(BucketSetup {
            default_ttl: Some(3_600),
            ..BucketSetup::admin("sec")
        })
        .await;

    let response = ctx
        .server
        .get(&format!("/api/v1/{bid}"))
        .authorization_bearer("sec")
        .await;
    response.assert_status_ok();
    let body: Value = response.json();
    assert_eq!(body.get("default_ttl"), Some(&json!(3_600)));
}

#[tokio::test]
async fn expired_key_not_in_listing() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::public()).await;
    let path = format!("/api/v1/{bid}/gone");

    ctx.server
        .post(&path)
        .add_query_param("ttl", "1")
        .text("x")
        .await
        .assert_status_ok();

    sleep(Duration::from_secs(2)).await;

    let list = ctx
        .server
        .get(&format!("/api/v1/{bid}/"))
        .add_header(header::ACCEPT, "application/json")
        .await;
    list.assert_status_ok();
    list.assert_json(&json!([]));
}
