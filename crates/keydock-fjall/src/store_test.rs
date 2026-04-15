//! Integration-style tests for [`crate::FjallStore`] (filesystem-backed).

use bytes::Bytes;
use keydock_domain::value::ValueKind;
use keydock_domain::{BucketId, BucketPolicy, Key, Permission, SigningKey, StoredValue};
use keydock_usecase::{BucketRepository, KeyRepository, hash_credential};
use pretty_assertions::assert_eq;
use secrecy::ExposeSecret;
use tempfile::tempdir;
use time::OffsetDateTime;

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

#[test]
fn kv_roundtrip_set_get_delete_isolated_by_bucket() {
    let dir = tempdir().expect("tempdir");
    let store = FjallStore::open(dir.path()).expect("open");
    let b1 = BucketId::new("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".to_string()).expect("id");
    let b2 = BucketId::new("ffffffff-gggg-hhhh-iiii-jjjjjjjjjjjj".to_string()).expect("id");
    let k = Key::from_bytes(Bytes::from_static(b"same-key")).expect("key");
    let v = StoredValue::new(Bytes::from_static(b"hello"), ValueKind::Utf8).expect("value");
    let exp = Some(OffsetDateTime::now_utc());
    KeyRepository::set(&store, &b1, &k, v.clone(), exp).expect("set b1");
    KeyRepository::set(&store, &b2, &k, v.clone(), None).expect("set b2");

    let e1 = KeyRepository::get(&store, &b1, &k)
        .expect("get")
        .expect("some");
    assert_eq!(e1.value, v);
    assert_eq!(e1.expires_at.is_some(), true);

    assert_eq!(KeyRepository::delete(&store, &b1, &k).expect("del"), true);
    assert_eq!(KeyRepository::get(&store, &b1, &k).expect("get"), None);
    assert_eq!(
        KeyRepository::delete(&store, &b1, &k).expect("del again"),
        false
    );

    let e2 = KeyRepository::get(&store, &b2, &k)
        .expect("get b2")
        .expect("some");
    assert_eq!(e2.value, v);
    assert_eq!(e2.expires_at, None);
}
