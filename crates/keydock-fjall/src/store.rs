use std::path::Path;
use std::sync::Arc;

use fjall::{Database, Keyspace, KeyspaceCreateOptions};
use keydock_domain::{BucketId, BucketPolicy};
use keydock_usecase::{BucketRepository, KeyRepository, UseCaseError};

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
    pub fn open(path: impl AsRef<Path>) -> Result<Self, FjallError> {
        let db = Arc::new(Database::builder(path).open()?);
        let meta = Arc::new(db.keyspace(META_KEYSPACE, KeyspaceCreateOptions::default)?);
        let data = Arc::new(db.keyspace(DATA_KEYSPACE, KeyspaceCreateOptions::default)?);
        Ok(Self { db, meta, data })
    }
}

impl BucketRepository for FjallStore {
    fn ping_metadata(&self) -> Result<(), UseCaseError> {
        Ok(())
    }

    fn get_policy(&self, bucket: &BucketId) -> Result<Option<BucketPolicy>, UseCaseError> {
        let key = bucket.as_str().as_bytes();
        match self
            .meta
            .get(key)
            .map_err(|e| UseCaseError::Storage(e.to_string()))?
        {
            Some(v) => {
                let bytes: &[u8] = v.as_ref();
                decode_policy(bytes)
                    .map(Some)
                    .map_err(|e| UseCaseError::Storage(e.to_string()))
            }
            None => Ok(None),
        }
    }

    fn create_bucket(&self, id: &BucketId, policy: BucketPolicy) -> Result<(), UseCaseError> {
        let key = id.as_str().as_bytes();
        let bytes = encode_policy(&policy).map_err(|e| UseCaseError::Storage(e.to_string()))?;
        self.meta
            .insert(key, bytes)
            .map_err(|e| UseCaseError::Storage(e.to_string()))
    }

    fn delete_bucket(&self, id: &BucketId) -> Result<(), UseCaseError> {
        let key = id.as_str().as_bytes();
        self.meta
            .remove(key)
            .map_err(|e| UseCaseError::Storage(e.to_string()))
    }
}

impl KeyRepository for FjallStore {
    fn not_implemented(&self, _bucket: &BucketId) -> Result<(), UseCaseError> {
        Err(UseCaseError::NotImplemented)
    }
}
