//! End-to-end smoke tests for `/metrics`.
//!
//! The Prometheus recorder is process-global and shared across tests, so
//! assertions compare *deltas* (before/after the action under test) rather
//! than absolute counter values.

use axum::http::header;
use bytes::Bytes;
use keydock_domain::{BucketId, Key};
use keydock_testkit::{BucketSetup, TestContext, api_error_body_json};
use pretty_assertions::assert_eq;

/// Extracts `value` from the Prometheus sample line matching `metric_name`
/// and all `label_fragments` (substring-AND, order-independent). Returns
/// `None` when no matching line exists.
fn sample_value(body: &str, metric_name: &str, label_fragments: &[&str]) -> Option<f64> {
    body.lines()
        .filter(|line| !line.starts_with('#'))
        .filter(|line| line.starts_with(metric_name))
        .filter(|line| label_fragments.iter().all(|needle| line.contains(needle)))
        .find_map(|line| line.rsplit_once(' ').and_then(|(_, v)| v.parse().ok()))
}

fn sample_value_or_zero(body: &str, metric_name: &str, label_fragments: &[&str]) -> f64 {
    sample_value(body, metric_name, label_fragments).unwrap_or(0.0)
}

#[tokio::test]
async fn metrics_endpoint_advertises_content_type_and_help_lines() {
    // `metrics-exporter-prometheus` skips `# HELP`/`# TYPE` for metrics that
    // have never recorded a sample, so drive each counter asserted below
    // before scraping `/metrics`.
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::admin("sec")).await;

    ctx.server.get("/health").await.assert_status_ok();
    ctx.server
        .put(&format!("/{bid}/warmup"))
        .authorization_bearer("sec")
        .text("seed")
        .await
        .assert_status_ok();

    let bucket = BucketId::new(bid.clone()).expect("valid bucket id from API");
    let key = Key::from_bytes(Bytes::copy_from_slice(b"warmup")).expect("valid key");
    ctx.corrupt_entry(&bucket, &key);
    ctx.server
        .get(&format!("/{bid}/warmup"))
        .authorization_bearer("sec")
        .await;

    let response = ctx.server.get("/metrics").await;
    response.assert_status_ok();
    response.assert_header(
        header::CONTENT_TYPE,
        "text/plain; version=0.0.4; charset=utf-8",
    );
    let body = response.text();

    // `gc_keys_expired_total` only surfaces after a real GC sweep tick,
    // which is covered by the TTL integration tests. Excluding it here
    // keeps the smoke test deterministic.
    for metric in [
        "http_requests_total",
        "http_request_duration_seconds",
        "storage_ops_total",
        "storage_errors_total",
    ] {
        assert_eq!(
            body.contains(&format!("# HELP {metric} ")),
            true,
            "missing `# HELP` for {metric}\n/metrics body:\n{body}"
        );
        assert_eq!(
            body.contains(&format!("# TYPE {metric} ")),
            true,
            "missing `# TYPE` for {metric}\n/metrics body:\n{body}"
        );
    }
}

#[tokio::test]
async fn http_requests_total_increments_on_health_traffic() {
    let ctx = TestContext::new();
    let metric = "http_requests_total";
    let labels = ["route=\"/health\"", "method=\"GET\"", "status=\"200\""];

    let before = sample_value_or_zero(&ctx.render_metrics().await, metric, &labels);

    const HITS: usize = 3;
    for _ in 0..HITS {
        ctx.server.get("/health").await.assert_status_ok();
    }

    let after = sample_value_or_zero(&ctx.render_metrics().await, metric, &labels);
    assert_eq!(
        after - before >= HITS as f64,
        true,
        "expected `{metric}{{route=/health,method=GET,status=200}}` to advance by \
         at least {HITS} (before={before}, after={after})"
    );
}

#[tokio::test]
async fn storage_ops_total_increments_on_successful_put() {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(BucketSetup::admin("sec")).await;
    let metric = "storage_ops_total";
    let labels = ["op=\"set\"", "result=\"ok\""];

    let before = sample_value_or_zero(&ctx.render_metrics().await, metric, &labels);

    ctx.server
        .put(&format!("/{bid}/hello"))
        .authorization_bearer("sec")
        .text("world")
        .await
        .assert_status_ok();

    let after = sample_value_or_zero(&ctx.render_metrics().await, metric, &labels);
    assert_eq!(
        after > before,
        true,
        "expected `{metric}{{op=set,result=ok}}` to increase (before={before}, after={after})"
    );
}

#[tokio::test]
async fn storage_errors_total_increments_on_codec_entry_failure() {
    let ctx = TestContext::new();
    let bid_text = ctx.create_bucket(BucketSetup::admin("sec")).await;
    let key_name = "doomed";

    ctx.server
        .put(&format!("/{bid_text}/{key_name}"))
        .authorization_bearer("sec")
        .text("seed")
        .await
        .assert_status_ok();

    let metric = "storage_errors_total";
    let labels = ["kind=\"codec_entry\""];
    let before = sample_value_or_zero(&ctx.render_metrics().await, metric, &labels);

    // Corruption → next GET fails at `decode_entry` → `FjallError::Codec`
    // → `From<FjallError> for UseCaseError` increments the counter before
    // the handler maps it to HTTP 500.
    let bucket = BucketId::new(bid_text.clone()).expect("valid bucket id from API");
    let key = Key::from_bytes(Bytes::copy_from_slice(key_name.as_bytes())).expect("valid key");
    ctx.corrupt_entry(&bucket, &key);

    let get = ctx
        .server
        .get(&format!("/{bid_text}/{key_name}"))
        .authorization_bearer("sec")
        .await;
    get.assert_status_internal_server_error();
    get.assert_json(&api_error_body_json(500, "internal_error"));

    let after = sample_value_or_zero(&ctx.render_metrics().await, metric, &labels);
    assert_eq!(
        after > before,
        true,
        "expected `{metric}{{kind=codec_entry}}` to advance (before={before}, after={after})"
    );
}
