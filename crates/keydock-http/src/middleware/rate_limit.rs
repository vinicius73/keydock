//! Fixed-window rate limiting per client IP.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::Request;
use axum::http::header::{HeaderName, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;
use keydock_config::RateLimitConfig;
use time::OffsetDateTime;

use crate::error::{internal_error, rate_limit_exceeded};

const WINDOW: Duration = Duration::from_secs(3600);

struct WindowState {
    count: u64,
    window_start: Instant,
    window_start_unix: i64,
}

/// Shared fixed-window counters keyed by client IP.
pub(crate) struct RateLimitState {
    windows: Mutex<HashMap<IpAddr, WindowState>>,
    config: RateLimitConfig,
}

impl RateLimitState {
    pub(crate) fn new(config: RateLimitConfig) -> Self {
        Self {
            windows: Mutex::new(HashMap::new()),
            config,
        }
    }
}

fn header_u64(name: &'static str, value: u64) -> Option<(HeaderName, HeaderValue)> {
    let name = HeaderName::from_static(name);
    let value = HeaderValue::from_str(&value.to_string()).ok()?;
    Some((name, value))
}

fn header_i64(name: &'static str, value: i64) -> Option<(HeaderName, HeaderValue)> {
    let name = HeaderName::from_static(name);
    let value = HeaderValue::from_str(&value.to_string()).ok()?;
    Some((name, value))
}

enum RateLimitDecision {
    Deny {
        limit: u64,
        reset_ts: i64,
    },
    Allow {
        limit: u64,
        remaining: u64,
        reset_ts: i64,
    },
}

fn apply_rate_limit_headers(
    mut resp: Response,
    limit: u64,
    remaining: u64,
    reset_unix: i64,
) -> Response {
    let headers = resp.headers_mut();
    if let Some((n, v)) = header_u64("x-ratelimit-limit", limit) {
        headers.insert(n, v);
    }
    if let Some((n, v)) = header_u64("x-ratelimit-remaining", remaining) {
        headers.insert(n, v);
    }
    if let Some((n, v)) = header_i64("x-ratelimit-reset", reset_unix) {
        headers.insert(n, v);
    }
    resp
}

/// Enforces `[rate_limit]` when enabled; adds `X-Ratelimit-*` headers when active.
pub async fn enforce_rate_limit(
    state: Arc<RateLimitState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    if !state.config.enabled {
        return next.run(req).await;
    }

    let limit = state.config.requests_per_hour;
    let ip = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0.ip())
        .unwrap_or(IpAddr::from([127, 0, 0, 1]));

    let decision = {
        let mut guard = match state.windows.lock() {
            Ok(g) => g,
            Err(_) => return internal_error(),
        };

        let now_unix = OffsetDateTime::now_utc().unix_timestamp();
        let entry = guard.entry(ip).or_insert_with(|| WindowState {
            count: 0,
            window_start: Instant::now(),
            window_start_unix: now_unix,
        });

        if entry.window_start.elapsed() >= WINDOW {
            entry.count = 0;
            entry.window_start = Instant::now();
            entry.window_start_unix = now_unix;
        }

        let reset_ts = entry.window_start_unix.saturating_add(3600);

        if entry.count >= limit {
            RateLimitDecision::Deny { limit, reset_ts }
        } else {
            entry.count = entry.count.saturating_add(1);
            let remaining = limit.saturating_sub(entry.count);
            RateLimitDecision::Allow {
                limit,
                remaining,
                reset_ts,
            }
        }
    };

    match decision {
        RateLimitDecision::Deny { limit, reset_ts } => {
            let resp = rate_limit_exceeded();
            apply_rate_limit_headers(resp, limit, 0, reset_ts)
        }
        RateLimitDecision::Allow {
            limit,
            remaining,
            reset_ts,
        } => {
            let resp = next.run(req).await;
            apply_rate_limit_headers(resp, limit, remaining, reset_ts)
        }
    }
}
