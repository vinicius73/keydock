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

#[derive(Deserialize)]
struct EntryCodecRef<'a> {
    payload: &'a [u8],
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
    postcard::to_allocvec(&codec).map_err(|e| FjallError::Codec(format!("postcard encode: {e}")))
}

fn expires_at_from_unix(ts: i64) -> Result<OffsetDateTime, FjallError> {
    OffsetDateTime::from_unix_timestamp(ts).map_err(|e| FjallError::Codec(e.to_string()))
}

pub fn decode_entry_expires_unix(bytes: &[u8]) -> Result<Option<i64>, FjallError> {
    let codec: EntryCodecRef<'_> = postcard::from_bytes(bytes)
        .map_err(|e| FjallError::Codec(format!("postcard decode: {e}")))?;
    Ok(codec.expires_unix)
}

pub fn decode_entry(bytes: &[u8]) -> Result<StoredEntry, FjallError> {
    let codec: EntryCodecRef<'_> = postcard::from_bytes(bytes)
        .map_err(|e| FjallError::Codec(format!("postcard decode: {e}")))?;
    let value = StoredValue::new(Bytes::copy_from_slice(codec.payload), codec.kind)
        .map_err(|e| FjallError::Codec(e.to_string()))?;
    let expires_at = match codec.expires_unix {
        None => None,
        Some(ts) => Some(expires_at_from_unix(ts)?),
    };
    Ok(StoredEntry { value, expires_at })
}

const EXPIRY_KEY_SEPARATOR: u8 = b':';
const EXPIRY_KEY_HEADER_LEN: usize = 8 + 1;

fn encode_ordered_i64(ts: i64) -> [u8; 8] {
    let bits = u64::from_be_bytes(ts.to_be_bytes()) ^ 0x8000_0000_0000_0000;
    bits.to_be_bytes()
}

fn decode_ordered_i64(bytes: [u8; 8]) -> i64 {
    let bits = u64::from_be_bytes(bytes) ^ 0x8000_0000_0000_0000;
    i64::from_be_bytes(bits.to_be_bytes())
}

/// Builds an expiry-index key from `(expires_unix, data_storage_key)`.
///
/// Format: `ordered_i64(expires_unix) + ':' + data_storage_key`.
pub fn expiry_index_key(expires_unix: i64, storage_key: &[u8]) -> Vec<u8> {
    let ts = encode_ordered_i64(expires_unix);
    let mut out = Vec::with_capacity(EXPIRY_KEY_HEADER_LEN + storage_key.len());
    out.extend_from_slice(&ts);
    out.push(EXPIRY_KEY_SEPARATOR);
    out.extend_from_slice(storage_key);
    out
}

pub fn parse_expiry_index_key(key: &[u8]) -> Option<(i64, &[u8])> {
    if key.len() <= EXPIRY_KEY_HEADER_LEN {
        return None;
    }
    let ts_bytes: [u8; 8] = key.get(..8)?.try_into().ok()?;
    if key.get(8).copied()? != EXPIRY_KEY_SEPARATOR {
        return None;
    }
    let storage_key = key.get(EXPIRY_KEY_HEADER_LEN..)?;
    Some((decode_ordered_i64(ts_bytes), storage_key))
}
