//! Integration-style tests for [`crate::FjallStore`] (filesystem-backed).

use bytes::Bytes;
use keydock_domain::value::ValueKind;
use keydock_domain::{BucketId, BucketPolicy, Key, Permission, SigningKey, StoredValue};
use keydock_usecase::{BucketRepository, KeyRepository, ListEntry, ListOpts, hash_credential};
use pretty_assertions::assert_eq;
use secrecy::ExposeSecret;
use std::time::Duration as SweepInterval;
use tempfile::tempdir;

use time::{Duration, OffsetDateTime};

use crate::FjallStore;

fn list_opts_no_prefix(now: OffsetDateTime) -> ListOpts<'static> {
    ListOpts {
        prefix: None,
        limit: 10_000,
        skip: 0,
        reverse: false,
        include_values: false,
        expires_before: Some(now),
    }
}

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

#[test]
fn list_empty_bucket_returns_empty() {
    let dir = tempdir().expect("tempdir");
    let store = FjallStore::open(dir.path()).expect("open");
    let bucket = BucketId::new("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".to_string()).expect("id");
    let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("ts");
    let opts = list_opts_no_prefix(now);
    let rows = KeyRepository::list(&store, &bucket, &opts).expect("list");
    let expected: Vec<ListEntry> = vec![];
    assert_eq!(rows, expected);
}

#[test]
fn list_returns_keys_in_lexicographic_order() {
    let dir = tempdir().expect("tempdir");
    let store = FjallStore::open(dir.path()).expect("open");
    let bucket = BucketId::new("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".to_string()).expect("id");
    let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("ts");
    let v = StoredValue::new(Bytes::from_static(b"v"), ValueKind::Utf8).expect("value");

    for name in [b"c", b"a", b"b"] {
        let k = Key::from_bytes(Bytes::copy_from_slice(name)).expect("key");
        KeyRepository::set(&store, &bucket, &k, v.clone(), None).expect("set");
    }

    let opts = list_opts_no_prefix(now);
    let rows = KeyRepository::list(&store, &bucket, &opts).expect("list");
    let keys: Vec<&[u8]> = rows.iter().map(|r| r.key.as_bytes()).collect();
    assert_eq!(
        keys,
        vec![b"a".as_slice(), b"b".as_slice(), b"c".as_slice()]
    );
}

#[test]
fn list_prefix_filters_user_keys() {
    let dir = tempdir().expect("tempdir");
    let store = FjallStore::open(dir.path()).expect("open");
    let bucket = BucketId::new("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".to_string()).expect("id");
    let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("ts");
    let v = StoredValue::new(Bytes::from_static(b"v"), ValueKind::Utf8).expect("value");

    for name in [b"foo:1", b"foo:2", b"bar:1"] {
        let k = Key::from_bytes(Bytes::copy_from_slice(name)).expect("key");
        KeyRepository::set(&store, &bucket, &k, v.clone(), None).expect("set");
    }

    let opts = ListOpts {
        prefix: Some(b"foo:"),
        limit: 10_000,
        skip: 0,
        reverse: false,
        include_values: false,
        expires_before: Some(now),
    };
    let rows = KeyRepository::list(&store, &bucket, &opts).expect("list");
    let keys: Vec<&[u8]> = rows.iter().map(|r| r.key.as_bytes()).collect();
    assert_eq!(keys, vec![b"foo:1".as_slice(), b"foo:2".as_slice()]);
}

#[test]
fn list_reverse_inverts_order() {
    let dir = tempdir().expect("tempdir");
    let store = FjallStore::open(dir.path()).expect("open");
    let bucket = BucketId::new("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".to_string()).expect("id");
    let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("ts");
    let v = StoredValue::new(Bytes::from_static(b"v"), ValueKind::Utf8).expect("value");

    for name in [b"a", b"b", b"c"] {
        let k = Key::from_bytes(Bytes::copy_from_slice(name)).expect("key");
        KeyRepository::set(&store, &bucket, &k, v.clone(), None).expect("set");
    }

    let opts = ListOpts {
        prefix: None,
        limit: 10_000,
        skip: 0,
        reverse: true,
        include_values: false,
        expires_before: Some(now),
    };
    let rows = KeyRepository::list(&store, &bucket, &opts).expect("list");
    let keys: Vec<&[u8]> = rows.iter().map(|r| r.key.as_bytes()).collect();
    assert_eq!(
        keys,
        vec![b"c".as_slice(), b"b".as_slice(), b"a".as_slice()]
    );
}

#[test]
fn list_skip_and_limit_paginate() {
    let dir = tempdir().expect("tempdir");
    let store = FjallStore::open(dir.path()).expect("open");
    let bucket = BucketId::new("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".to_string()).expect("id");
    let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("ts");
    let v = StoredValue::new(Bytes::from_static(b"v"), ValueKind::Utf8).expect("value");

    for name in [b"k0", b"k1", b"k2", b"k3"] {
        let k = Key::from_bytes(Bytes::copy_from_slice(name)).expect("key");
        KeyRepository::set(&store, &bucket, &k, v.clone(), None).expect("set");
    }

    let opts = ListOpts {
        prefix: None,
        limit: 2,
        skip: 1,
        reverse: false,
        include_values: false,
        expires_before: Some(now),
    };
    let rows = KeyRepository::list(&store, &bucket, &opts).expect("list");
    let keys: Vec<&[u8]> = rows.iter().map(|r| r.key.as_bytes()).collect();
    assert_eq!(keys, vec![b"k1".as_slice(), b"k2".as_slice()]);
}

#[test]
fn list_excludes_expired_entries_when_expires_before_set() {
    let dir = tempdir().expect("tempdir");
    let store = FjallStore::open(dir.path()).expect("open");
    let bucket = BucketId::new("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".to_string()).expect("id");
    let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("ts");
    let v = StoredValue::new(Bytes::from_static(b"v"), ValueKind::Utf8).expect("value");

    let k_live = Key::from_bytes(Bytes::from_static(b"live")).expect("key");
    let k_dead = Key::from_bytes(Bytes::from_static(b"dead")).expect("key");
    KeyRepository::set(
        &store,
        &bucket,
        &k_live,
        v.clone(),
        Some(now + Duration::seconds(10)),
    )
    .expect("set live");
    KeyRepository::set(
        &store,
        &bucket,
        &k_dead,
        v,
        Some(now - Duration::seconds(1)),
    )
    .expect("set dead");

    let opts = ListOpts {
        prefix: None,
        limit: 10_000,
        skip: 0,
        reverse: false,
        include_values: false,
        expires_before: Some(now),
    };
    let rows = KeyRepository::list(&store, &bucket, &opts).expect("list");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].key.as_bytes(), b"live");

    let dead_stored = KeyRepository::get(&store, &bucket, &k_dead)
        .expect("get")
        .expect("dead key still in storage");
    assert_eq!(dead_stored.expires_at, Some(now - Duration::seconds(1)));
}

#[test]
fn gc_sweep_removes_expired_keys_from_storage() {
    let dir = tempdir().expect("tempdir");
    let store = FjallStore::open(dir.path()).expect("open");
    let bucket = BucketId::new("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".to_string()).expect("id");
    let now = OffsetDateTime::now_utc();
    let v = StoredValue::new(Bytes::from_static(b"v"), ValueKind::Utf8).expect("value");

    let k_expired = Key::from_bytes(Bytes::from_static(b"gone")).expect("key");
    let k_ok = Key::from_bytes(Bytes::from_static(b"stay")).expect("key");
    KeyRepository::set(
        &store,
        &bucket,
        &k_expired,
        v.clone(),
        Some(now - Duration::seconds(60)),
    )
    .expect("set expired");
    KeyRepository::set(&store, &bucket, &k_ok, v, Some(now + Duration::hours(1))).expect("set ok");

    assert_eq!(
        KeyRepository::get(&store, &bucket, &k_expired)
            .expect("get")
            .is_some(),
        true
    );

    let sweeper = store.build_gc_sweeper(SweepInterval::from_secs(3600));
    sweeper.sweep_once();

    assert_eq!(
        KeyRepository::get(&store, &bucket, &k_expired).expect("get"),
        None
    );
    assert_eq!(
        KeyRepository::get(&store, &bucket, &k_ok)
            .expect("get")
            .is_some(),
        true
    );
}
