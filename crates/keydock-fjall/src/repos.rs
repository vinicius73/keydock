//! Postcard codec and layout keys for the `data` keyspace.

use bytes::Bytes;
use keydock_domain::value::ValueKind;
use keydock_domain::{BucketId, Key, StoredValue};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use keydock_usecase::StoredEntry;

use crate::FjallError;

/// Layout: `{bucket_id_utf8}:{user_key_bytes}`. Bucket ids are UUIDs (no `:`), so the separator is unambiguous.
pub fn data_storage_key(bucket: &BucketId, key: &Key) -> Vec<u8> {
    let mut out = Vec::with_capacity(bucket.as_str().len() + 1 + key.as_bytes().len());
    out.extend_from_slice(bucket.as_str().as_bytes());
    out.push(b':');
    out.extend_from_slice(key.as_bytes());
    out
}

/// Prefix for [`fjall::Keyspace::prefix`] scans: `{bucket_id}:` plus optional user-key prefix bytes.
pub fn data_key_prefix(bucket: &BucketId, user_prefix: Option<&[u8]>) -> Vec<u8> {
    let mut out =
        Vec::with_capacity(bucket.as_str().len() + 1 + user_prefix.map(<[u8]>::len).unwrap_or(0));
    out.extend_from_slice(bucket.as_str().as_bytes());
    out.push(b':');
    if let Some(p) = user_prefix {
        out.extend_from_slice(p);
    }
    out
}

/// Parses the user [`Key`] from a full storage key given the bucket id.
pub fn user_key_from_storage_key(storage_key: &[u8], bucket: &BucketId) -> Option<Key> {
    let b = bucket.as_str().as_bytes();
    let rest = storage_key.strip_prefix(b)?;
    let rest = rest.strip_prefix(b":")?;
    Key::from_bytes(Bytes::copy_from_slice(rest)).ok()
}

#[derive(Serialize, Deserialize)]
struct EntryCodec {
    payload: Vec<u8>,
    kind: ValueKind,
    expires_unix: Option<i64>,
}

pub fn encode_entry(
    value: &StoredValue,
    expires_at: Option<OffsetDateTime>,
) -> Result<Vec<u8>, FjallError> {
    let expires_unix = expires_at.map(|t| t.unix_timestamp());
    let codec = EntryCodec {
        payload: value.payload.as_ref().to_vec(),
        kind: value.kind,
        expires_unix,
    };
    postcard::to_allocvec(&codec).map_err(|e| FjallError::Adapter(format!("postcard encode: {e}")))
}

pub fn decode_entry(bytes: &[u8]) -> Result<StoredEntry, FjallError> {
    let codec: EntryCodec = postcard::from_bytes(bytes)
        .map_err(|e| FjallError::Adapter(format!("postcard decode: {e}")))?;
    let value = StoredValue::new(Bytes::from(codec.payload), codec.kind)
        .map_err(|e| FjallError::Adapter(e.to_string()))?;
    let expires_at = match codec.expires_unix {
        None => None,
        Some(ts) => Some(
            OffsetDateTime::from_unix_timestamp(ts)
                .map_err(|e| FjallError::Adapter(e.to_string()))?,
        ),
    };
    Ok(StoredEntry { value, expires_at })
}
