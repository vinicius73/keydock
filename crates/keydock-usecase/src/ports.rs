use keydock_domain::BucketId;

use crate::UseCaseError;

/// Persistence port for bucket metadata (implemented by `keydock-fjall`).
pub trait BucketRepository: Send + Sync {
    fn ping_metadata(&self) -> Result<(), UseCaseError>;
}

/// Persistence port for key-value payloads (implemented by `keydock-fjall`).
pub trait KeyRepository: Send + Sync {
    fn not_implemented(&self, bucket: &BucketId) -> Result<(), UseCaseError>;
}
