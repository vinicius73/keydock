use keydock_domain::{BucketId, BucketPolicy, CounterOp, Key, StoredValue};
use time::OffsetDateTime;

use crate::UseCaseError;
use crate::keys::StoredEntry;

/// One step in an atomic multi-key transaction (`KeyRepository::apply_batch`).
#[derive(Debug, Clone, PartialEq)]
pub enum TxnOp {
    Set {
        key: Key,
        value: StoredValue,
        expires_at: Option<OffsetDateTime>,
    },
    Delete {
        key: Key,
    },
}

/// Parameters for listing keys within a bucket (callers apply HTTP/query defaults via `KeyService::list`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListOpts<'a> {
    pub prefix: Option<&'a [u8]>,
    pub limit: usize,
    pub skip: usize,
    pub reverse: bool,
    pub include_values: bool,
    /// Entries with `expires_at <= expires_before` are excluded. `None` disables expiry filtering.
    pub expires_before: Option<OffsetDateTime>,
}

/// One row in a bucket listing.
#[derive(Debug, Clone, PartialEq)]
pub struct ListEntry {
    pub key: Key,
    pub value: Option<StoredValue>,
}

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

    fn list(&self, bucket: &BucketId, opts: &ListOpts<'_>) -> Result<Vec<ListEntry>, UseCaseError>;

    /// Atomically read-modify-write a numeric key (counter). Implemented with storage-level locking.
    fn increment(
        &self,
        bucket: &BucketId,
        key: &Key,
        op: CounterOp,
        expires_at: Option<OffsetDateTime>,
    ) -> Result<StoredValue, UseCaseError>;

    /// Apply multiple key operations atomically (all succeed or none).
    fn apply_batch(&self, bucket: &BucketId, ops: &[TxnOp]) -> Result<(), UseCaseError>;
}
