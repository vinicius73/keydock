//! Key/value orchestration: type inference, TTL selection, repository delegation.

use bytes::Bytes;
use keydock_domain::value::ValueKind;
use keydock_domain::{BucketId, CounterOp, DomainError, Key, StoredValue};
use keydock_support::Clock;
use time::{Duration, OffsetDateTime};
use tracing::instrument;

use crate::UseCaseError;
use crate::ports::{KeyRepository, ListEntry, ListOpts};

/// Stored key payload plus optional expiry metadata (`KeyService::get` treats expired entries as missing).
#[derive(Debug, Clone, PartialEq)]
pub struct StoredEntry {
    pub value: StoredValue,
    pub expires_at: Option<OffsetDateTime>,
}

/// Raw listing inputs before defaults (HTTP layer builds this from query params).
#[derive(Debug, Clone, Default)]
pub struct ListOptsInput {
    pub prefix: Option<Vec<u8>>,
    pub limit: Option<usize>,
    pub skip: Option<usize>,
    pub reverse: Option<bool>,
    pub include_values: Option<bool>,
}

/// Stateless orchestrator for key operations (handlers inject `KeyRepository` + `Clock`).
pub struct KeyService;

impl KeyService {
    #[instrument(
        skip_all,
        name = "KeyService::get",
        fields(bucket = %bucket.as_str(), key_len = key.as_bytes().len())
    )]
    pub fn get(
        repo: &dyn KeyRepository,
        clock: &dyn Clock,
        bucket: &BucketId,
        key: &Key,
    ) -> Result<StoredEntry, UseCaseError> {
        let entry = repo.get(bucket, key)?.ok_or(UseCaseError::NotFound)?;
        if let Some(exp) = entry.expires_at
            && exp <= clock.now_utc()
        {
            tracing::debug!(
                bucket = %bucket.as_str(),
                key_len = key.as_bytes().len(),
                "key read skipped (expired ttl)"
            );
            return Err(UseCaseError::NotFound);
        }
        Ok(entry)
    }

    #[instrument(
        skip_all,
        name = "KeyService::list",
        fields(bucket = %bucket.as_str())
    )]
    pub fn list(
        repo: &dyn KeyRepository,
        clock: &dyn Clock,
        bucket: &BucketId,
        input: ListOptsInput,
    ) -> Result<Vec<ListEntry>, UseCaseError> {
        const DEFAULT_LIMIT: usize = 10_000;
        let opts = ListOpts {
            prefix: input.prefix.as_deref(),
            limit: input.limit.unwrap_or(DEFAULT_LIMIT),
            skip: input.skip.unwrap_or(0),
            reverse: input.reverse.unwrap_or(false),
            include_values: input.include_values.unwrap_or(false),
            expires_before: Some(clock.now_utc()),
        };
        repo.list(bucket, &opts)
    }

    /// Maps HTTP write inputs (body, `Content-Type`, TTL) to storage; kept explicit for reviewability.
    #[allow(clippy::too_many_arguments)]
    #[instrument(
        skip_all,
        name = "KeyService::set",
        fields(
            bucket = %bucket.as_str(),
            key_len = key.as_bytes().len(),
            has_ttl_override = ttl_override.is_some(),
            has_default_ttl = default_ttl.is_some(),
        )
    )]
    pub fn set(
        repo: &dyn KeyRepository,
        clock: &dyn Clock,
        bucket: &BucketId,
        key: &Key,
        body: Bytes,
        content_type: Option<&str>,
        ttl_override: Option<u64>,
        default_ttl: Option<u64>,
    ) -> Result<StoredValue, UseCaseError> {
        let value = Self::infer_stored_value(body, content_type)?;
        let expires_at = Self::resolve_ttl(clock, ttl_override, default_ttl)?;
        repo.set(bucket, key, value.clone(), expires_at)?;
        Ok(value)
    }

    /// Infers [`StoredValue`] from raw bytes and optional `Content-Type` (same rules as `set`).
    #[instrument(skip_all, name = "KeyService::infer_stored_value")]
    pub fn infer_stored_value(
        body: Bytes,
        content_type: Option<&str>,
    ) -> Result<StoredValue, UseCaseError> {
        let kind = infer_value_kind(body.as_ref(), content_type);
        Ok(StoredValue::new(body, kind)?)
    }

    /// Resolves TTL seconds into an absolute expiry instant (`None` or `0` → no expiry).
    #[instrument(skip_all, name = "KeyService::resolve_ttl")]
    pub fn resolve_ttl(
        clock: &dyn Clock,
        ttl_override: Option<u64>,
        default_ttl: Option<u64>,
    ) -> Result<Option<OffsetDateTime>, UseCaseError> {
        let effective = ttl_override.or(default_ttl);
        match effective {
            None | Some(0) => Ok(None),
            Some(secs) => {
                let secs_i64 = i64::try_from(secs).map_err(|_| {
                    UseCaseError::Domain(DomainError::InvalidTtl(
                        "ttl seconds out of supported range".into(),
                    ))
                })?;
                let at = clock
                    .now_utc()
                    .checked_add(Duration::seconds(secs_i64))
                    .ok_or_else(|| {
                        UseCaseError::Domain(DomainError::InvalidTtl(
                            "ttl resulting instant out of range".into(),
                        ))
                    })?;
                Ok(Some(at))
            }
        }
    }

    /// Atomically applies a counter delta; storage enforces numeric semantics and locking.
    #[instrument(
        skip_all,
        name = "KeyService::increment",
        fields(
            bucket = %bucket.as_str(),
            key_len = key.as_bytes().len(),
            has_ttl_override = ttl_override.is_some(),
            has_default_ttl = default_ttl.is_some(),
        )
    )]
    pub fn increment(
        repo: &dyn KeyRepository,
        clock: &dyn Clock,
        bucket: &BucketId,
        key: &Key,
        op: CounterOp,
        ttl_override: Option<u64>,
        default_ttl: Option<u64>,
    ) -> Result<StoredValue, UseCaseError> {
        let expires_at = Self::resolve_ttl(clock, ttl_override, default_ttl)?;
        repo.increment(bucket, key, op, expires_at)
    }

    #[instrument(
        skip_all,
        name = "KeyService::delete",
        fields(bucket = %bucket.as_str(), key_len = key.as_bytes().len())
    )]
    pub fn delete(
        repo: &dyn KeyRepository,
        bucket: &BucketId,
        key: &Key,
    ) -> Result<(), UseCaseError> {
        if repo.delete(bucket, key)? {
            Ok(())
        } else {
            Err(UseCaseError::NotFound)
        }
    }
}

fn content_type_is_json(ct: &str) -> bool {
    let lower = ct.to_ascii_lowercase();
    lower.contains("application/json")
}

fn infer_value_kind(body: &[u8], content_type: Option<&str>) -> ValueKind {
    if let Some(ct) = content_type
        && content_type_is_json(ct)
    {
        return ValueKind::Json;
    }
    if let Ok(s) = std::str::from_utf8(body) {
        let t = s.trim();
        if t.parse::<i64>().is_ok() {
            return ValueKind::Int64;
        }
        if t.parse::<f64>().is_ok() {
            return ValueKind::Float64;
        }
        if serde_json::from_str::<serde_json::Value>(t).is_ok() {
            return ValueKind::Json;
        }
        return ValueKind::Utf8;
    }
    ValueKind::Raw
}

#[cfg(test)]
mod tests {
    use keydock_domain::DomainError;
    use keydock_domain::value::ValueKind;
    use keydock_support::clock::SystemClock;
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use crate::ports::{ListEntry, ListOpts};

    use super::*;

    /// Fixed instant for deterministic TTL tests.
    #[derive(Clone, Copy)]
    struct MockClock(OffsetDateTime);

    impl Clock for MockClock {
        fn now_utc(&self) -> OffsetDateTime {
            self.0
        }
    }

    struct MockRepo {
        last_expires: std::sync::Mutex<Option<Option<OffsetDateTime>>>,
    }

    impl KeyRepository for MockRepo {
        fn get(&self, _bucket: &BucketId, _key: &Key) -> Result<Option<StoredEntry>, UseCaseError> {
            Ok(None)
        }

        fn set(
            &self,
            _bucket: &BucketId,
            _key: &Key,
            _value: StoredValue,
            expires_at: Option<OffsetDateTime>,
        ) -> Result<(), UseCaseError> {
            *self.last_expires.lock().expect("lock") = Some(expires_at);
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

        fn apply_batch(
            &self,
            _bucket: &BucketId,
            _ops: &[crate::ports::TxnOp],
        ) -> Result<(), UseCaseError> {
            Err(UseCaseError::NotImplemented)
        }
    }

    fn bucket() -> BucketId {
        BucketId::new("b".to_string()).expect("id")
    }

    fn key() -> Key {
        Key::from_bytes(Bytes::from_static(b"k")).expect("key")
    }

    #[rstest]
    #[case(b"hello", None, ValueKind::Utf8)]
    #[case(b"42", None, ValueKind::Int64)]
    #[case(b"3.14", None, ValueKind::Float64)]
    #[case(br#"{"a":1}"#, None, ValueKind::Json)]
    #[case::explicit_json_header(br#"x"#, Some("application/json"), ValueKind::Json)]
    #[case::explicit_json_header(
        br#"not-json"#,
        Some("application/json; charset=utf-8"),
        ValueKind::Json
    )]
    #[case(b"true", None, ValueKind::Json)]
    #[case(b"", None, ValueKind::Utf8)]
    #[case(b"\xff\xfe", None, ValueKind::Raw)]
    fn infer_value_kind_cases(
        #[case] body: &[u8],
        #[case] ct: Option<&str>,
        #[case] expected: ValueKind,
    ) {
        assert_eq!(infer_value_kind(body, ct), expected);
    }

    #[test]
    fn set_empty_body_is_utf8() {
        let repo = MockRepo {
            last_expires: std::sync::Mutex::new(None),
        };
        let clock = SystemClock;
        let bid = bucket();
        let k = key();
        let v =
            KeyService::set(&repo, &clock, &bid, &k, Bytes::new(), None, None, None).expect("set");
        assert_eq!(v.kind, ValueKind::Utf8);
        assert_eq!(v.payload.as_ref(), b"");
        assert_eq!(*repo.last_expires.lock().expect("lock"), Some(None));
    }

    #[test]
    fn set_ttl_override_over_default() {
        let repo = MockRepo {
            last_expires: std::sync::Mutex::new(None),
        };
        let clock = SystemClock;
        let bid = bucket();
        let k = key();
        let v = KeyService::set(
            &repo,
            &clock,
            &bid,
            &k,
            Bytes::from_static(b"1"),
            None,
            Some(60),
            Some(3600),
        )
        .expect("set");
        assert_eq!(v.kind, ValueKind::Int64);
        let exp = (*repo.last_expires.lock().expect("lock")).expect("set called");
        assert_eq!(exp.is_some(), true);
    }

    #[test]
    fn set_rejects_ttl_seconds_out_of_i64_range() {
        let repo = MockRepo {
            last_expires: std::sync::Mutex::new(None),
        };
        let clock = SystemClock;
        let bid = bucket();
        let k = key();
        let err = KeyService::set(
            &repo,
            &clock,
            &bid,
            &k,
            Bytes::from_static(b"1"),
            None,
            Some(u64::MAX),
            None,
        )
        .expect_err("ttl overflow");
        assert_eq!(
            matches!(err, UseCaseError::Domain(DomainError::InvalidTtl(_))),
            true
        );
    }

    /// Returns a fixed entry from `get` for TTL enforcement tests.
    struct RepoWithEntry {
        entry: Option<StoredEntry>,
    }

    impl KeyRepository for RepoWithEntry {
        fn get(&self, _bucket: &BucketId, _key: &Key) -> Result<Option<StoredEntry>, UseCaseError> {
            Ok(self.entry.clone())
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

        fn apply_batch(
            &self,
            _bucket: &BucketId,
            _ops: &[crate::ports::TxnOp],
        ) -> Result<(), UseCaseError> {
            Err(UseCaseError::NotImplemented)
        }
    }

    fn sample_entry(expires_at: Option<OffsetDateTime>) -> StoredEntry {
        StoredEntry {
            value: StoredValue::new(Bytes::from_static(b"x"), ValueKind::Utf8).expect("value"),
            expires_at,
        }
    }

    #[test]
    fn get_expired_entry_returns_not_found() {
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("ts");
        let clock = MockClock(now);
        let repo = RepoWithEntry {
            entry: Some(sample_entry(Some(now - Duration::seconds(1)))),
        };
        let bid = bucket();
        let k = key();
        let err = KeyService::get(&repo, &clock, &bid, &k).expect_err("expired");
        assert_eq!(matches!(err, UseCaseError::NotFound), true);
    }

    #[test]
    fn get_future_expiry_returns_entry() {
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("ts");
        let clock = MockClock(now);
        let entry = sample_entry(Some(now + Duration::seconds(60)));
        let repo = RepoWithEntry {
            entry: Some(entry.clone()),
        };
        let bid = bucket();
        let k = key();
        let got = KeyService::get(&repo, &clock, &bid, &k).expect("get");
        assert_eq!(got, entry);
    }

    #[test]
    fn get_no_expiry_returns_entry() {
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("ts");
        let clock = MockClock(now);
        let entry = sample_entry(None);
        let repo = RepoWithEntry {
            entry: Some(entry.clone()),
        };
        let bid = bucket();
        let k = key();
        let got = KeyService::get(&repo, &clock, &bid, &k).expect("get");
        assert_eq!(got, entry);
    }
}
