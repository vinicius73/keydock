//! Key/value orchestration: type inference, TTL selection, repository delegation.

use bytes::Bytes;
use keydock_domain::value::ValueKind;
use keydock_domain::{BucketId, Key, StoredValue};
use keydock_support::Clock;
use time::{Duration, OffsetDateTime};

use crate::UseCaseError;
use crate::ports::KeyRepository;

/// Stored key payload plus optional expiry metadata (enforcement is M2).
#[derive(Debug, Clone, PartialEq)]
pub struct StoredEntry {
    pub value: StoredValue,
    pub expires_at: Option<OffsetDateTime>,
}

/// Stateless orchestrator for key operations (handlers inject `KeyRepository` + `Clock`).
pub struct KeyService;

impl KeyService {
    pub fn get(
        repo: &dyn KeyRepository,
        bucket: &BucketId,
        key: &Key,
    ) -> Result<StoredEntry, UseCaseError> {
        repo.get(bucket, key)?.ok_or(UseCaseError::NotFound)
    }

    /// Maps HTTP write inputs (body, `Content-Type`, TTL) to storage; kept explicit for reviewability.
    #[allow(clippy::too_many_arguments)]
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
        let kind = infer_value_kind(body.as_ref(), content_type);
        let value = StoredValue::new(body, kind)?;
        let effective = ttl_override.or(default_ttl);
        let expires_at = match effective {
            None | Some(0) => None,
            Some(secs) => Some(clock.now_utc() + Duration::seconds(secs as i64)),
        };
        repo.set(bucket, key, value.clone(), expires_at)?;
        Ok(value)
    }

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
    use keydock_domain::value::ValueKind;
    use keydock_support::clock::SystemClock;
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;

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
        assert!(exp.is_some());
    }
}
