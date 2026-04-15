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
}

impl KeyRepository for FjallStore {
    fn not_implemented(&self, _bucket: &BucketId) -> Result<(), UseCaseError> {
        Err(UseCaseError::NotImplemented)
    }
}

#[cfg(test)]
mod tests {
    use keydock_domain::Permission;
    use keydock_usecase::BucketRepository;
    use keydock_usecase::hash_credential;
    use pretty_assertions::assert_eq;
    use secrecy::ExposeSecret;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn create_bucket_roundtrips_policy() {
        let dir = tempdir().expect("tempdir");
        let store = FjallStore::open(dir.path()).expect("open");
        let bucket = BucketId::new("my-bucket".to_string()).expect("id");
        let rk =
            keydock_domain::SigningKey::new(Box::new(b"root-key-test-32-bytes-min!!".to_vec()));
        let secret_hash = hash_credential("secret", &rk);
        assert_ne!(secret_hash, b"secret".to_vec());
        let policy = BucketPolicy {
            default_ttl_secs: Some(3600),
            anonymous_access: Permission::READ_ONLY,
            secret_key_hash: Some(secret_hash),
            read_key_hash: None,
            write_key_hash: None,
            signing_key: Some(keydock_domain::SigningKey::new(Box::new(
                b"sign-key".to_vec(),
            ))),
            signing_key_generation: 2,
        };
        store
            .create_bucket(&bucket, policy.clone())
            .expect("create");
        let loaded = store.get_policy(&bucket).expect("get").expect("some");
        assert_eq!(loaded.default_ttl_secs, policy.default_ttl_secs);
        assert_eq!(loaded.anonymous_access, policy.anonymous_access);
        assert_eq!(loaded.secret_key_hash, policy.secret_key_hash);
        assert_eq!(loaded.signing_key_generation, policy.signing_key_generation);
        assert_eq!(
            loaded.signing_key.as_ref().unwrap().expose_secret(),
            policy.signing_key.as_ref().unwrap().expose_secret()
        );
    }
}
