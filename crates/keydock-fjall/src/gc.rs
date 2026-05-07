//! Garbage collection: removes expired key entries from the `data` keyspace.

use std::sync::Arc;
use std::time::{Duration, Instant};

use fjall::Keyspace;
use time::OffsetDateTime;
use tokio_util::sync::CancellationToken;
use tracing::instrument;

use crate::locks::StripedLocks;
use crate::repos::{decode_entry_expires_unix, parse_expiry_index_key};

/// Periodically scans the expiry index and deletes entries whose TTL has passed.
pub struct GcSweeper {
    data: Arc<Keyspace>,
    expiry: Arc<Keyspace>,
    write_locks: Arc<StripedLocks>,
    interval: Duration,
}

impl GcSweeper {
    pub(crate) fn new(
        data: Arc<Keyspace>,
        expiry: Arc<Keyspace>,
        write_locks: Arc<StripedLocks>,
        interval: Duration,
    ) -> Self {
        Self {
            data,
            expiry,
            write_locks,
            interval,
        }
    }

    /// Removes all entries with `expires_at <= now` (same logic as listing expiry).
    pub fn sweep_once(&self) {
        sweep_keyspace(&self.data, &self.expiry, &self.write_locks);
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
                    let expiry = Arc::clone(&self.expiry);
                    let write_locks = Arc::clone(&self.write_locks);
                    let sweep = tokio::task::spawn_blocking(move || sweep_keyspace(&data, &expiry, &write_locks));
                    if let Err(e) = sweep.await {
                        tracing::warn!(error = %e, "gc: sweep task join failed");
                    }
                }
            }
        }
    }
}

fn sweep_keyspace(data: &Arc<Keyspace>, expiry: &Arc<Keyspace>, write_locks: &Arc<StripedLocks>) {
    let started = Instant::now();
    let now_ts = OffsetDateTime::now_utc().unix_timestamp();
    let mut expiry_scanned = 0u64;
    let mut expiry_removed = 0u64;
    let mut keys_removed = 0u64;

    for guard in expiry.iter() {
        expiry_scanned += 1;
        let (uk, uv) = match guard.into_inner() {
            Ok(kv) => kv,
            Err(e) => {
                tracing::warn!(error = %e, "gc: read entry during sweep");
                continue;
            }
        };

        drop(uv);

        let expiry_key = uk.as_ref();
        let Some((expires_unix, storage_key)) = parse_expiry_index_key(expiry_key) else {
            tracing::debug!("gc: skip malformed expiry index key");
            continue;
        };

        if expires_unix > now_ts {
            break;
        }

        let _guard = match write_locks.lock_for(storage_key) {
            Ok(g) => g,
            Err(e) => {
                tracing::warn!(error = %e, "gc: write lock failed");
                continue;
            }
        };

        match expiry.remove(expiry_key) {
            Ok(()) => {
                expiry_removed += 1;
            }
            Err(e) => {
                tracing::warn!(error = %e, "gc: remove expiry index key failed");
                continue;
            }
        }

        let Some(v) = (match data.get(storage_key) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(error = %e, "gc: read entry for expiry verification failed");
                continue;
            }
        }) else {
            continue;
        };

        let Some(actual_unix) = (match decode_entry_expires_unix(v.as_ref()) {
            Ok(v) => v,
            Err(_) => {
                tracing::debug!("gc: skip entry that failed to decode");
                continue;
            }
        }) else {
            continue;
        };
        if actual_unix > now_ts {
            continue;
        }
        if actual_unix != expires_unix {
            continue;
        }

        match data.remove(storage_key) {
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
        metrics::counter!("gc_keys_expired_total").increment(keys_removed);
        tracing::info!(
            expiry_scanned,
            expiry_removed,
            keys_removed,
            elapsed_ms,
            "gc sweep removed expired keys"
        );
    } else {
        tracing::debug!(
            expiry_scanned,
            expiry_removed,
            elapsed_ms,
            "gc sweep completed"
        );
    }
}

#[cfg(test)]
mod tests {
    use fjall::{Database, KeyspaceCreateOptions};
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    use crate::layout::EXPIRY_KEYSPACE;
    use crate::repos::{expiry_index_key, parse_expiry_index_key};

    #[test]
    fn expiry_keyspace_iterates_in_ascending_key_order() {
        let dir = tempdir().expect("tempdir");
        let db = Database::builder(dir.path()).open().expect("open");
        let expiry = db
            .keyspace(EXPIRY_KEYSPACE, KeyspaceCreateOptions::default)
            .expect("keyspace");

        for ts in [100_i64, 10, 50] {
            let storage_key = format!("k{ts}").into_bytes();
            let idx = expiry_index_key(ts, &storage_key);
            expiry.insert(&idx, []).expect("insert");
        }

        let mut prev: Option<i64> = None;
        for guard in expiry.iter() {
            let (uk, _uv) = guard.into_inner().expect("iter");
            let (ts, _storage_key) =
                parse_expiry_index_key(uk.as_ref()).expect("parse expiry index key");
            if let Some(prev) = prev {
                assert_eq!(ts >= prev, true, "ts must be nondecreasing");
            }
            prev = Some(ts);
        }
    }
}
