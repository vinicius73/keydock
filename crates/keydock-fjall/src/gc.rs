//! Garbage collection: removes expired key entries from the `data` keyspace.

use std::sync::Arc;
use std::time::{Duration, Instant};

use fjall::Keyspace;
use time::OffsetDateTime;
use tokio_util::sync::CancellationToken;
use tracing::instrument;

use crate::repos::decode_entry;

/// Periodically scans the `data` keyspace and deletes entries whose TTL has passed.
pub struct GcSweeper {
    data: Arc<Keyspace>,
    interval: Duration,
}

impl GcSweeper {
    pub(crate) fn new(data: Arc<Keyspace>, interval: Duration) -> Self {
        Self { data, interval }
    }

    /// Removes all entries with `expires_at <= now` (same logic as listing expiry).
    pub fn sweep_once(&self) {
        sweep_keyspace(&self.data);
    }

    /// Runs until `cancel` is triggered, sleeping `interval` between sweeps.
    #[instrument(skip_all, name = "GcSweeper::run")]
    pub async fn run(self, cancel: CancellationToken) {
        tracing::info!(
            interval_secs = self.interval.as_secs(),
            "gc sweeper task running"
        );
        loop {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => {
                    tracing::info!("gc sweeper stopped");
                    return;
                }
                _ = tokio::time::sleep(self.interval) => {
                    let data = Arc::clone(&self.data);
                    let sweep = tokio::task::spawn_blocking(move || sweep_keyspace(&data));
                    if let Err(e) = sweep.await {
                        tracing::warn!(error = %e, "gc: sweep task join failed");
                    }
                }
            }
        }
    }
}

fn sweep_keyspace(data: &Arc<Keyspace>) {
    let started = Instant::now();
    let now_ts = OffsetDateTime::now_utc().unix_timestamp();
    let mut keys_scanned = 0u64;
    let mut keys_removed = 0u64;

    for guard in data.iter() {
        keys_scanned += 1;
        let (uk, uv) = match guard.into_inner() {
            Ok(kv) => kv,
            Err(e) => {
                tracing::warn!(error = %e, "gc: read entry during sweep");
                continue;
            }
        };

        let Ok(entry) = decode_entry(uv.as_ref()) else {
            tracing::debug!("gc: skip entry that failed to decode");
            continue;
        };

        let Some(exp) = entry.expires_at else {
            continue;
        };

        if exp.unix_timestamp() > now_ts {
            continue;
        }

        let key_bytes = uk.as_ref();
        match data.remove(key_bytes) {
            Ok(()) => {
                keys_removed += 1;
            }
            Err(e) => {
                tracing::warn!(error = %e, "gc: remove expired entry failed");
            }
        }
    }

    let elapsed_ms = started.elapsed().as_millis() as u64;
    if keys_removed > 0 {
        tracing::info!(
            keys_scanned,
            keys_removed,
            elapsed_ms,
            "gc sweep removed expired keys"
        );
    } else {
        tracing::debug!(keys_scanned, elapsed_ms, "gc sweep completed");
    }
}
