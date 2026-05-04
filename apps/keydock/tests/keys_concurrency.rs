//! Concurrent HTTP access to the same bucket (integration).

use std::sync::Arc;

use axum::http::header;
use futures_util::future::join_all;
use pretty_assertions::assert_eq;
use serde_json::json;

use keydock_testkit::{BucketSetup, TestContext};

#[tokio::test]
async fn parallel_counter_patch_no_lost_increments() {
    let ctx = Arc::new(TestContext::new());
    let bid = ctx.create_bucket(BucketSetup::restricted("r", "w")).await;
    let path = format!("/api/v1/{bid}/hot");

    let patch_futures: Vec<_> = (0..32)
        .map(|_| {
            let ctx = Arc::clone(&ctx);
            let path = path.clone();
            async move {
                ctx.server
                    .patch(&path)
                    .authorization_bearer("w")
                    .text("+1")
                    .await
            }
        })
        .collect();

    let results = join_all(patch_futures).await;
    assert_eq!(results.len(), 32);
    for res in results {
        res.assert_status_ok();
    }

    let finalv = ctx.server.get(&path).authorization_bearer("r").await;
    finalv.assert_status_ok();
    finalv.assert_header(header::CONTENT_TYPE, "text/plain; charset=utf-8");
    assert_eq!(finalv.text(), "32");
}

#[tokio::test]
async fn parallel_txn_writes_distinct_keys() {
    let ctx = Arc::new(TestContext::new());
    let bid = ctx.create_bucket(BucketSetup::admin("sec")).await;
    let n = 16usize;

    let txn_futures: Vec<_> = (0..n)
        .map(|i| {
            let ctx = Arc::clone(&ctx);
            let post_path = format!("/api/v1/{bid}");
            async move {
                let res = ctx
                    .server
                    .post(&post_path)
                    .authorization_bearer("sec")
                    .json(&json!({
                        "txn": [{ "set": format!("k{i}"), "value": format!("v{i}") }]
                    }))
                    .await;
                res.assert_status_no_content();
            }
        })
        .collect();

    join_all(txn_futures).await;

    for i in 0..n {
        let get = ctx
            .server
            .get(&format!("/api/v1/{bid}/k{i}"))
            .authorization_bearer("sec")
            .await;
        get.assert_status_ok();
        get.assert_header(header::CONTENT_TYPE, "text/plain; charset=utf-8");
        assert_eq!(get.text(), format!("v{i}"));
    }
}
