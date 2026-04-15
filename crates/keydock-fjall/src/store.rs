use std::path::Path;
use std::sync::Arc;

use fjall::{Database, Keyspace, KeyspaceCreateOptions};
use keydock_domain::{BucketId, BucketPolicy, Key, StoredValue};
use keydock_usecase::{
    BucketRepository, KeyRepository, ListEntry, ListOpts, StoredEntry, UseCaseError,
};
use time::OffsetDateTime;
use tracing::instrument;

use crate::FjallError;
use crate::codec::{decode_policy, encode_policy};
use crate::layout::{DATA_KEYSPACE, META_KEYSPACE};
use crate::repos::{
    data_key_prefix, data_storage_key, decode_entry, encode_entry, user_key_from_storage_key,
};

/// Owns the Fjall [`Database`] handle and keyspaces used by the product.
#[derive(Clone)]
pub struct FjallStore {
    #[allow(dead_code)]
    db: Arc<Database>,
    meta: Arc<Keyspace>,
    data: Arc<Keyspace>,
}

impl FjallStore {
    #[instrument(skip_all, name = "FjallStore::open")]
    pub fn open(path: impl AsRef<Path>) -> Result<Self, FjallError> {
        let db = Arc::new(Database::builder(path).open()?);
        let meta = Arc::new(db.keyspace(META_KEYSPACE, KeyspaceCreateOptions::default)?);
        let data = Arc::new(db.keyspace(DATA_KEYSPACE, KeyspaceCreateOptions::default)?);
        Ok(Self { db, meta, data })
    }
}

impl BucketRepository for FjallStore {
    #[instrument(skip_all, name = "FjallStore::ping_metadata")]
    fn ping_metadata(&self) -> Result<(), UseCaseError> {
        Ok(())
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
        let k = data_storage_key(bucket, key);
        match self.data.get(&k).map_err(FjallError::from)? {
            None => Ok(None),
            Some(v) => {
                let bytes: &[u8] = v.as_ref();
                let entry = decode_entry(bytes)?;
                Ok(Some(entry))
            }
        }
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
        let k = data_storage_key(bucket, key);
        let bytes = encode_entry(&value, expires_at)?;
        self.data.insert(&k, bytes).map_err(FjallError::from)?;
        Ok(())
    }

    #[instrument(
        skip_all,
        name = "FjallStore::delete",
        fields(bucket = %bucket.as_str(), key_len = key.as_bytes().len())
    )]
    fn delete(&self, bucket: &BucketId, key: &Key) -> Result<bool, UseCaseError> {
        let k = data_storage_key(bucket, key);
        let existed = self.data.contains_key(&k).map_err(FjallError::from)?;
        if existed {
            self.data.remove(&k).map_err(FjallError::from)?;
        }
        Ok(existed)
    }

    #[instrument(
        skip_all,
        name = "FjallStore::list",
        fields(bucket = %bucket.as_str())
    )]
    fn list(&self, bucket: &BucketId, opts: &ListOpts<'_>) -> Result<Vec<ListEntry>, UseCaseError> {
        let scan_prefix = data_key_prefix(bucket, opts.prefix);
        let mut collected: Vec<ListEntry> = Vec::new();

        for guard in self.data.prefix(scan_prefix) {
            let (uk, uv) = guard.into_inner().map_err(FjallError::from)?;
            let storage_key = uk.as_ref();
            let Some(user_key) = user_key_from_storage_key(storage_key, bucket) else {
                tracing::debug!("skipped malformed data key in list scan");
                continue;
            };

            let entry = decode_entry(uv.as_ref())?;

            if let (Some(cutoff), Some(exp)) = (opts.expires_before, entry.expires_at)
                && exp <= cutoff
            {
                continue;
            }

            let value = if opts.include_values {
                Some(entry.value)
            } else {
                None
            };
            collected.push(ListEntry {
                key: user_key,
                value,
            });
        }

        if opts.reverse {
            collected.reverse();
        }

        let out: Vec<ListEntry> = collected
            .into_iter()
            .skip(opts.skip)
            .take(opts.limit)
            .collect();
        Ok(out)
    }
}
