use std::path::Path;
use std::sync::Arc;

use fjall::{Database, Keyspace, KeyspaceCreateOptions};
use keydock_domain::{BucketId, BucketPolicy};
use keydock_usecase::{BucketRepository, KeyRepository, UseCaseError};

use crate::FjallError;

const DEFAULT_KEYSPACE: &str = "keydock_main";

/// Owns the Fjall [`Database`] handle and keyspace used by the product.
#[derive(Clone)]
pub struct FjallStore {
    #[allow(dead_code)]
    db: Arc<Database>,
    #[allow(dead_code)]
    keyspace: Arc<Keyspace>,
}

impl FjallStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, FjallError> {
        let db = Arc::new(Database::builder(path).open()?);
        let ks = Arc::new(db.keyspace(DEFAULT_KEYSPACE, KeyspaceCreateOptions::default)?);
        Ok(Self { db, keyspace: ks })
    }
}

impl BucketRepository for FjallStore {
    fn ping_metadata(&self) -> Result<(), UseCaseError> {
        Ok(())
    }

    fn get_policy(&self, _bucket: &BucketId) -> Result<Option<BucketPolicy>, UseCaseError> {
        Err(UseCaseError::NotImplemented)
    }

    fn create_bucket(&self, _id: &BucketId, _policy: BucketPolicy) -> Result<(), UseCaseError> {
        Err(UseCaseError::NotImplemented)
    }
}

impl KeyRepository for FjallStore {
    fn not_implemented(&self, _bucket: &BucketId) -> Result<(), UseCaseError> {
        Err(UseCaseError::NotImplemented)
    }
}
