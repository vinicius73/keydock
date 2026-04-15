//! Multi-key transaction orchestration.

use keydock_domain::BucketId;
use tracing::instrument;

use crate::UseCaseError;
use crate::ports::{KeyRepository, TxnOp};

/// Executes a batch of pre-validated operations (auth is checked by HTTP handlers).
pub struct TxnService;

impl TxnService {
    /// Persists all operations in one atomic batch.
    #[instrument(
        skip_all,
        name = "TxnService::execute",
        fields(bucket = %bucket.as_str(), op_count = ops.len())
    )]
    pub fn execute(
        repo: &dyn KeyRepository,
        bucket: &BucketId,
        ops: &[TxnOp],
    ) -> Result<(), UseCaseError> {
        repo.apply_batch(bucket, ops)
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use keydock_domain::value::ValueKind;
    use keydock_domain::{BucketId, CounterOp, Key, StoredValue};
    use pretty_assertions::assert_eq;
    use time::OffsetDateTime;

    use crate::keys::StoredEntry;
    use crate::ports::{KeyRepository, ListEntry, ListOpts, TxnOp};

    use super::*;

    struct OkBatchRepo;

    impl KeyRepository for OkBatchRepo {
        fn get(&self, _bucket: &BucketId, _key: &Key) -> Result<Option<StoredEntry>, UseCaseError> {
            Ok(None)
        }

        fn set(
            &self,
            _bucket: &BucketId,
            _key: &Key,
            _value: StoredValue,
            _expires_at: Option<OffsetDateTime>,
        ) -> Result<(), UseCaseError> {
            Ok(())
        }

        fn delete(&self, _bucket: &BucketId, _key: &Key) -> Result<bool, UseCaseError> {
            Ok(false)
        }

        fn list(
            &self,
            _bucket: &BucketId,
            _opts: &ListOpts<'_>,
        ) -> Result<Vec<ListEntry>, UseCaseError> {
            Ok(vec![])
        }

        fn increment(
            &self,
            _bucket: &BucketId,
            _key: &Key,
            _op: CounterOp,
            _expires_at: Option<OffsetDateTime>,
        ) -> Result<StoredValue, UseCaseError> {
            Err(UseCaseError::NotImplemented)
        }

        fn apply_batch(&self, _bucket: &BucketId, _ops: &[TxnOp]) -> Result<(), UseCaseError> {
            Ok(())
        }
    }

    #[test]
    fn execute_delegates_to_apply_batch() {
        let repo = OkBatchRepo;
        let bid = BucketId::new("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".to_string()).expect("id");
        let k = Key::from_bytes(Bytes::from_static(b"k")).expect("key");
        let v = StoredValue::new(Bytes::from_static(b"v"), ValueKind::Utf8).expect("v");
        let ops = vec![TxnOp::Set {
            key: k,
            value: v,
            expires_at: None,
        }];
        assert_eq!(TxnService::execute(&repo, &bid, &ops).is_ok(), true);
    }
}
