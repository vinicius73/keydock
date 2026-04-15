use std::path::Path;
use std::sync::Arc;

use fjall::{Database, Keyspace, KeyspaceCreateOptions};
use keydock_domain::{BucketId, BucketPolicy};
use keydock_usecase::{BucketRepository, KeyRepository, UseCaseError};
use tracing::instrument;

use crate::FjallError;
use crate::codec::{decode_policy, encode_policy};
use crate::layout::{DATA_KEYSPACE, META_KEYSPACE};

/// Owns the Fjall [`Database`] handle and keyspaces used by the product.
#[derive(Clone)]
pub struct FjallStore {
    #[allow(dead_code)]
    db: Arc<Database>,
    meta: Arc<Keyspace>,
    #[allow(dead_code)]
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
        name = "FjallStore::not_implemented",
        fields(bucket = %_bucket.as_str())
    )]
    fn not_implemented(&self, _bucket: &BucketId) -> Result<(), UseCaseError> {
        Err(UseCaseError::NotImplemented)
    }
}
