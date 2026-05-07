use std::time::Instant;

use axum::body::Body;
use axum::extract::MatchedPath;
use axum::http::Request;
use axum::middleware::Next;
use axum::response::Response;

/// Records Prometheus counters and histograms for each HTTP request.
pub async fn track_http_metrics(req: Request<Body>, next: Next) -> Response {
    let path = req
        .extensions()
        .get::<MatchedPath>()
        .map(|p| p.as_str().to_owned())
        .unwrap_or_else(|| req.uri().path().to_owned());
    let method = req.method().as_str().to_owned();
    let start = Instant::now();
    let resp = next.run(req).await;
    let status = resp.status().as_u16().to_string();
    let duration_secs = start.elapsed().as_secs_f64();

    metrics::counter!(
        "http_requests_total",
        "route" => path.clone(),
        "method" => method.clone(),
        "status" => status
    )
    .increment(1);
    metrics::histogram!("http_request_duration_seconds", "route" => path, "method" => method)
        .record(duration_secs);

    resp
}
