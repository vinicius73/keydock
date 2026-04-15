use keydock_domain::{BucketId, BucketPolicy, Key, StoredValue};
use time::OffsetDateTime;

use crate::UseCaseError;
use crate::keys::StoredEntry;

/// Persistence port for bucket metadata (implemented by `keydock-fjall`).
pub trait BucketRepository: Send + Sync {
    fn ping_metadata(&self) -> Result<(), UseCaseError>;

    fn get_policy(&self, bucket: &BucketId) -> Result<Option<BucketPolicy>, UseCaseError>;

    fn create_bucket(&self, id: &BucketId, policy: BucketPolicy) -> Result<(), UseCaseError>;

    fn delete_bucket(&self, id: &BucketId) -> Result<(), UseCaseError>;
}

/// Persistence port for key-value payloads (implemented by `keydock-fjall`).
pub trait KeyRepository: Send + Sync {
    fn get(&self, bucket: &BucketId, key: &Key) -> Result<Option<StoredEntry>, UseCaseError>;

    fn set(
        &self,
        bucket: &BucketId,
        key: &Key,
        value: StoredValue,
        expires_at: Option<OffsetDateTime>,
    ) -> Result<(), UseCaseError>;

    fn delete(&self, bucket: &BucketId, key: &Key) -> Result<bool, UseCaseError>;
}
