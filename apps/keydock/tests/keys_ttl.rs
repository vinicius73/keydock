//! TTL behavior (HTTP integration).

use std::time::Duration;

use axum::http::header;
use keydock_testkit::{BucketSetup, TestContext};
use serde_json::json;
use tokio::time::sleep;

#[tokio::test]
async fn get_returns_404_after_ttl_expires() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::public()).await;
    let path = format!("/{bid}/ttl1");

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
    let path = format!("/{bid}/ttl-renew");

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
async fn expired_key_not_in_listing() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::public()).await;
    let path = format!("/{bid}/gone");

    ctx.server
        .post(&path)
        .add_query_param("ttl", "1")
        .text("x")
        .await
        .assert_status_ok();

    sleep(Duration::from_secs(2)).await;

    let list = ctx
        .server
        .get(&format!("/{bid}/"))
        .add_header(header::ACCEPT, "application/json")
        .await;
    list.assert_status_ok();
    list.assert_json(&json!([]));
}
