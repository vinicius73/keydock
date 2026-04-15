use keydock_domain::{BucketId, BucketPolicy};

use crate::UseCaseError;

/// Persistence port for bucket metadata (implemented by `keydock-fjall`).
pub trait BucketRepository: Send + Sync {
    fn ping_metadata(&self) -> Result<(), UseCaseError>;

    fn get_policy(&self, bucket: &BucketId) -> Result<Option<BucketPolicy>, UseCaseError>;

    fn create_bucket(&self, id: &BucketId, policy: BucketPolicy) -> Result<(), UseCaseError>;
}

/// Persistence port for key-value payloads (implemented by `keydock-fjall`).
pub trait KeyRepository: Send + Sync {
    fn not_implemented(&self, bucket: &BucketId) -> Result<(), UseCaseError>;
}
