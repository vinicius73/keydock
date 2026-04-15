//! Integration-style tests for [`crate::FjallStore`] (filesystem-backed).

use keydock_domain::{BucketId, BucketPolicy, Permission, SigningKey};
use keydock_usecase::{BucketRepository, hash_credential};
use pretty_assertions::assert_eq;
use secrecy::ExposeSecret;
use tempfile::tempdir;

use crate::FjallStore;

#[test]
fn create_bucket_roundtrips_policy() {
    let dir = tempdir().expect("tempdir");
    let store = FjallStore::open(dir.path()).expect("open");
    let bucket = BucketId::new("my-bucket".to_string()).expect("id");
    let rk = SigningKey::new(Box::new(b"root-key-test-32-bytes-min!!".to_vec()));
    let secret_hash = hash_credential("secret", &rk).expect("hash credential");
    assert_ne!(secret_hash, b"secret".to_vec());
    let policy = BucketPolicy {
        default_ttl_secs: Some(3600),
        anonymous_access: Permission::READ_ONLY,
        secret_key_hash: Some(secret_hash),
        read_key_hash: None,
        write_key_hash: None,
        signing_key: Some(SigningKey::new(Box::new(b"sign-key".to_vec()))),
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
        loaded
            .signing_key
            .as_ref()
            .map(|k| k.expose_secret().as_slice()),
        policy
            .signing_key
            .as_ref()
            .map(|k| k.expose_secret().as_slice())
    );
}
