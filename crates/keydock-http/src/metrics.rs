//! Catalog of `# HELP`/`# TYPE` metadata for the first-party metrics.
//!
//! Kept as a single entry point so the binary and the integration testkit
//! stay in sync. Must be invoked once, right after the Prometheus recorder
//! is installed — otherwise `/metrics` renders samples without metadata.

use metrics::{Unit, describe_counter, describe_histogram};
use tracing::instrument;

/// Registers metadata for every metric emitted by the product. Idempotent.
#[instrument(skip_all)]
pub fn describe_all() {
    describe_counter!(
        "http_requests_total",
        Unit::Count,
        "Total HTTP requests handled, labeled by matched route, method and status code."
    );
    describe_histogram!(
        "http_request_duration_seconds",
        Unit::Seconds,
        "HTTP request duration in seconds, labeled by matched route and method."
    );
    describe_counter!(
        "storage_ops_total",
        Unit::Count,
        "Total storage operations executed against the persistence layer, labeled by op and result (ok|err)."
    );
    describe_counter!(
        "storage_errors_total",
        Unit::Count,
        "Total storage errors surfaced to use cases, labeled by kind (backend|adapter|codec_policy|codec_entry)."
    );
    describe_counter!(
        "gc_keys_expired_total",
        Unit::Count,
        "Total expired keys removed by the background GC sweeper."
    );
}
