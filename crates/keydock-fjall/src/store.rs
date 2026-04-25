use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use fjall::{Database, Keyspace, KeyspaceCreateOptions};
use keydock_domain::{BucketId, BucketPolicy, CounterOp, CounterValue, Key, StoredValue};
use keydock_usecase::{
    BucketRepository, KeyRepository, ListEntry, ListOpts, StoredEntry, TxnOp, UseCaseError,
};
use time::OffsetDateTime;
use tracing::instrument;

use crate::FjallError;
use crate::codec::{decode_policy, encode_policy};
use crate::gc::GcSweeper;
use crate::layout::{DATA_KEYSPACE, EXPIRY_KEYSPACE, META_KEYSPACE};
use crate::locks::StripedLocks;
use crate::repos::{
    data_key_prefix, data_storage_key, decode_entry, decode_entry_expires_unix, encode_entry,
    expiry_index_key, user_key_from_storage_key,
};

/// Above this many keys after the expiry filter, reverse listing logs a warning.
const REVERSE_LIST_WARN_ENTRY_COUNT: usize = 50_000;

fn record_storage_op<T>(op: &'static str, result: &Result<T, UseCaseError>) {
    let label = if result.is_ok() { "ok" } else { "err" };
    metrics::counter!("storage_ops_total", "op" => op, "result" => label).increment(1);
}

/// Owns the Fjall [`Database`] handle and keyspaces used by the product.
#[derive(Clone)]
pub struct FjallStore {
    db: Arc<Database>,
    meta: Arc<Keyspace>,
    data: Arc<Keyspace>,
    expiry: Arc<Keyspace>,
    /// Serializes conflicting writes for the same storage key.
    write_locks: Arc<StripedLocks>,
}

impl FjallStore {
    #[instrument(skip_all, name = "FjallStore::open")]
    pub fn open(path: impl AsRef<Path>) -> Result<Self, FjallError> {
        let db = Arc::new(Database::builder(path).open()?);
        let meta = Arc::new(db.keyspace(META_KEYSPACE, KeyspaceCreateOptions::default)?);
        let data = Arc::new(db.keyspace(DATA_KEYSPACE, KeyspaceCreateOptions::default)?);
        let expiry = Arc::new(db.keyspace(EXPIRY_KEYSPACE, KeyspaceCreateOptions::default)?);
        Ok(Self {
            db,
            meta,
            data,
            expiry,
            write_locks: Arc::new(StripedLocks::new()),
        })
    }

    #[instrument(skip_all, name = "FjallStore::build_gc_sweeper")]
    pub fn build_gc_sweeper(&self, interval: Duration) -> GcSweeper {
        GcSweeper::new(
            Arc::clone(&self.data),
            Arc::clone(&self.expiry),
            Arc::clone(&self.write_locks),
            interval,
        )
    }

    /// Writes arbitrary bytes at the data-keyspace storage key for
    /// `(bucket, key)`, bypassing the entry codec.
    ///
    /// Used by integration tests to simulate on-disk corruption and drive
    /// `storage_errors_total{kind="codec_entry"}`. Feature-gated so this
    /// surface is compiled out of production binaries.
    #[cfg(any(test, feature = "testkit"))]
    #[instrument(
        skip_all,
        name = "FjallStore::testkit_write_raw",
        fields(bucket = %bucket.as_str(), key_len = key.as_bytes().len())
    )]
    pub fn testkit_write_raw(
        &self,
        bucket: &BucketId,
        key: &Key,
        raw: &[u8],
    ) -> Result<(), FjallError> {
        let storage_key = data_storage_key(bucket, key);
        self.data.insert(&storage_key, raw)?;
        Ok(())
    }
}

impl BucketRepository for FjallStore {
    #[instrument(skip_all, name = "FjallStore::ping_metadata")]
    fn ping_metadata(&self) -> Result<(), UseCaseError> {
        let result = (|| -> Result<(), UseCaseError> {
            self.meta.get(b"__ping__").map_err(FjallError::from)?;
            Ok(())
        })();
        record_storage_op("ping_metadata", &result);
        result
    }

    #[instrument(
        skip_all,
        name = "FjallStore::get_policy",
        fields(bucket = %bucket.as_str())
    )]
    fn get_policy(&self, bucket: &BucketId) -> Result<Option<BucketPolicy>, UseCaseError> {
        let key = bucket.as_str().as_bytes();
        match self.meta.get(key).map_err(FjallError::from)? {
            Some(v) => {
                let bytes: &[u8] = v.as_ref();
                Ok(Some(decode_policy(bytes)?))
            }
            None => Ok(None),
        }
    }

    #[instrument(
        skip_all,
        name = "FjallStore::create_bucket",
        fields(bucket = %id.as_str())
    )]
    fn create_bucket(&self, id: &BucketId, policy: BucketPolicy) -> Result<(), UseCaseError> {
        let key = id.as_str().as_bytes();
        let bytes = encode_policy(&policy)?;
        self.meta.insert(key, bytes).map_err(FjallError::from)?;
        Ok(())
    }

    #[instrument(
        skip_all,
        name = "FjallStore::delete_bucket",
        fields(bucket = %id.as_str())
    )]
    fn delete_bucket(&self, id: &BucketId) -> Result<(), UseCaseError> {
        let key = id.as_str().as_bytes();
        self.meta.remove(key).map_err(FjallError::from)?;
        Ok(())
    }
}

impl KeyRepository for FjallStore {
    #[instrument(
        skip_all,
        name = "FjallStore::get",
        fields(bucket = %bucket.as_str(), key_len = key.as_bytes().len())
    )]
    fn get(&self, bucket: &BucketId, key: &Key) -> Result<Option<StoredEntry>, UseCaseError> {
        let result = (|| -> Result<Option<StoredEntry>, UseCaseError> {
            let k = data_storage_key(bucket, key);
            match self.data.get(&k).map_err(FjallError::from)? {
                None => Ok(None),
                Some(v) => {
                    let bytes: &[u8] = v.as_ref();
                    let entry = decode_entry(bytes)?;
                    Ok(Some(entry))
                }
            }
        })();
        record_storage_op("get", &result);
        result
    }

    #[instrument(
        skip_all,
        name = "FjallStore::set",
        fields(bucket = %bucket.as_str(), key_len = key.as_bytes().len())
    )]
    fn set(
        &self,
        bucket: &BucketId,
        key: &Key,
        value: StoredValue,
        expires_at: Option<OffsetDateTime>,
    ) -> Result<(), UseCaseError> {
        let result = (|| -> Result<(), UseCaseError> {
            let k = data_storage_key(bucket, key);
            let _guard = self.write_locks.lock_for(&k)?;
            let bytes = encode_entry(&value, expires_at)?;
            if let Some(exp) = expires_at {
                let idx = expiry_index_key(exp.unix_timestamp(), &k);
                let mut batch = self.db.batch();
                batch.insert(self.data.as_ref(), k, bytes);
                batch.insert(self.expiry.as_ref(), idx, []);
                batch.commit().map_err(FjallError::from)?;
            } else {
                self.data.insert(&k, bytes).map_err(FjallError::from)?;
            }
            Ok(())
        })();
        record_storage_op("set", &result);
        result
    }

    #[instrument(
        skip_all,
        name = "FjallStore::delete",
        fields(bucket = %bucket.as_str(), key_len = key.as_bytes().len())
    )]
    fn delete(&self, bucket: &BucketId, key: &Key) -> Result<bool, UseCaseError> {
        let result = (|| -> Result<bool, UseCaseError> {
            let k = data_storage_key(bucket, key);
            let _guard = self.write_locks.lock_for(&k)?;
            let existed = self.data.contains_key(&k).map_err(FjallError::from)?;
            if existed {
                self.data.remove(&k).map_err(FjallError::from)?;
            }
            Ok(existed)
        })();
        record_storage_op("delete", &result);
        result
    }

    #[instrument(
        skip_all,
        name = "FjallStore::list",
        fields(bucket = %bucket.as_str())
    )]
    fn list(&self, bucket: &BucketId, opts: &ListOpts<'_>) -> Result<Vec<ListEntry>, UseCaseError> {
        let result = self.list_inner(bucket, opts);
        record_storage_op("list", &result);
        result
    }

    #[instrument(
        skip_all,
        name = "FjallStore::increment",
        fields(bucket = %bucket.as_str(), key_len = key.as_bytes().len())
    )]
    fn increment(
        &self,
        bucket: &BucketId,
        key: &Key,
        op: CounterOp,
        expires_at: Option<OffsetDateTime>,
    ) -> Result<StoredValue, UseCaseError> {
        let result = (|| -> Result<StoredValue, UseCaseError> {
            let k = data_storage_key(bucket, key);
            let _guard = self.write_locks.lock_for(&k)?;
            let now = OffsetDateTime::now_utc();
            let current = match self.data.get(&k).map_err(FjallError::from)? {
                None => CounterValue::Int(0),
                Some(v) => {
                    let entry = decode_entry(v.as_ref())?;
                    if let Some(exp) = entry.expires_at
                        && exp <= now
                    {
                        CounterValue::Int(0)
                    } else {
                        CounterValue::from_stored(&entry.value)?
                    }
                }
            };
            let merged = op.apply(current)?;
            let stored = merged.into_stored()?;
            let bytes = encode_entry(&stored, expires_at)?;
            if let Some(exp) = expires_at {
                let idx = expiry_index_key(exp.unix_timestamp(), &k);
                let mut batch = self.db.batch();
                batch.insert(self.data.as_ref(), k, bytes);
                batch.insert(self.expiry.as_ref(), idx, []);
                batch.commit().map_err(FjallError::from)?;
            } else {
                self.data.insert(&k, bytes).map_err(FjallError::from)?;
            }
            Ok(stored)
        })();
        record_storage_op("increment", &result);
        result
    }

    #[instrument(skip_all, name = "FjallStore::apply_batch", fields(bucket = %bucket.as_str()))]
    fn apply_batch(&self, bucket: &BucketId, ops: &[TxnOp]) -> Result<(), UseCaseError> {
        let result = (|| -> Result<(), UseCaseError> {
            if ops.is_empty() {
                return Ok(());
            }

            enum BatchItem {
                Set {
                    storage_key: Vec<u8>,
                    bytes: Vec<u8>,
                    idx: Option<Vec<u8>>,
                },
                Delete {
                    storage_key: Vec<u8>,
                },
            }

            let mut items: Vec<BatchItem> = Vec::with_capacity(ops.len());
            for op in ops {
                match op {
                    TxnOp::Set {
                        key,
                        value,
                        expires_at,
                    } => {
                        let storage_key = data_storage_key(bucket, key);
                        let idx = expires_at.map(|exp| {
                            expiry_index_key(exp.unix_timestamp(), storage_key.as_slice())
                        });
                        let bytes = encode_entry(value, *expires_at)?;
                        items.push(BatchItem::Set {
                            storage_key,
                            bytes,
                            idx,
                        });
                    }
                    TxnOp::Delete { key } => {
                        let storage_key = data_storage_key(bucket, key);
                        items.push(BatchItem::Delete { storage_key });
                    }
                }
            }

            let _guards =
                self.write_locks
                    .lock_stripes_for_keys(items.iter().map(|it| match it {
                        BatchItem::Set { storage_key, .. } | BatchItem::Delete { storage_key } => {
                            storage_key.as_slice()
                        }
                    }))?;

            let mut batch = self.db.batch();
            for it in items {
                match it {
                    BatchItem::Set {
                        storage_key,
                        bytes,
                        idx,
                    } => {
                        batch.insert(self.data.as_ref(), storage_key, bytes);
                        if let Some(idx) = idx {
                            batch.insert(self.expiry.as_ref(), idx, []);
                        }
                    }
                    BatchItem::Delete { storage_key } => {
                        batch.remove(self.data.as_ref(), storage_key);
                    }
                }
            }
            batch.commit().map_err(FjallError::from)?;
            Ok(())
        })();
        record_storage_op("apply_batch", &result);
        result
    }
}

impl FjallStore {
    fn list_inner(
        &self,
        bucket: &BucketId,
        opts: &ListOpts<'_>,
    ) -> Result<Vec<ListEntry>, UseCaseError> {
        if opts.limit == 0 {
            return Ok(vec![]);
        }

        let scan_prefix = data_key_prefix(bucket, opts.prefix);
        let cutoff_unix = opts.expires_before.map(|t| t.unix_timestamp());

        if opts.reverse {
            let window = opts.skip.saturating_add(opts.limit);
            if window == 0 {
                return Ok(vec![]);
            }

            // Fjall prefix scan is forward-only; keep only the last `skip + limit` entries so memory is bounded.
            let mut matched = 0usize;
            let mut deque: std::collections::VecDeque<ListEntry> =
                std::collections::VecDeque::with_capacity(window);
            for guard in self.data.prefix(scan_prefix) {
                let (uk, uv) = guard.into_inner().map_err(FjallError::from)?;
                let storage_key = uk.as_ref();
                let Some(user_key) = user_key_from_storage_key(storage_key, bucket) else {
                    tracing::debug!("skipped malformed data key in list scan");
                    continue;
                };

                if opts.include_values {
                    let entry = decode_entry(uv.as_ref())?;
                    if let (Some(cutoff), Some(exp)) = (opts.expires_before, entry.expires_at)
                        && exp <= cutoff
                    {
                        continue;
                    }
                    matched += 1;
                    deque.push_back(ListEntry {
                        key: user_key,
                        value: Some(entry.value),
                    });
                } else {
                    let expires_unix = decode_entry_expires_unix(uv.as_ref())?;
                    if let (Some(cutoff), Some(exp)) = (cutoff_unix, expires_unix)
                        && exp <= cutoff
                    {
                        continue;
                    }
                    matched += 1;
                    deque.push_back(ListEntry {
                        key: user_key,
                        value: None,
                    });
                }

                while deque.len() > window {
                    deque.pop_front();
                }
            }

            if matched >= REVERSE_LIST_WARN_ENTRY_COUNT {
                tracing::warn!(
                    bucket = %bucket.as_str(),
                    matched,
                    "reverse listing scanned a large key set"
                );
            }

            let out: Vec<ListEntry> = deque
                .into_iter()
                .rev()
                .skip(opts.skip)
                .take(opts.limit)
                .collect();
            return Ok(out);
        }

        let mut skip_remaining = opts.skip;
        let mut taken = 0usize;
        let limit = opts.limit;
        let mut out: Vec<ListEntry> = Vec::new();

        for guard in self.data.prefix(scan_prefix) {
            if taken >= limit {
                break;
            }
            let (uk, uv) = guard.into_inner().map_err(FjallError::from)?;
            let storage_key = uk.as_ref();
            let Some(user_key) = user_key_from_storage_key(storage_key, bucket) else {
                tracing::debug!("skipped malformed data key in list scan");
                continue;
            };

            if opts.include_values {
                let entry = decode_entry(uv.as_ref())?;
                if let (Some(cutoff), Some(exp)) = (opts.expires_before, entry.expires_at)
                    && exp <= cutoff
                {
                    continue;
                }

                if skip_remaining > 0 {
                    skip_remaining -= 1;
                    continue;
                }

                out.push(ListEntry {
                    key: user_key,
                    value: Some(entry.value),
                });
                taken += 1;
                continue;
            }

            let expires_unix = decode_entry_expires_unix(uv.as_ref())?;
            if let (Some(cutoff), Some(exp)) = (cutoff_unix, expires_unix)
                && exp <= cutoff
            {
                continue;
            }

            if skip_remaining > 0 {
                skip_remaining -= 1;
                continue;
            }

            out.push(ListEntry {
                key: user_key,
                value: None,
            });
            taken += 1;
        }

        Ok(out)
    }
}
