use bytes::Bytes;
use serde::de::Error;
use serde::{Deserialize, Deserializer, Serializer};

pub fn serialize<S>(bytes: &Bytes, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_bytes(bytes.as_ref())
}

pub fn deserialize_key<'de, D>(deserializer: D) -> Result<Bytes, D::Error>
where
    D: Deserializer<'de>,
{
    let v = <Vec<u8>>::deserialize(deserializer)?;
    if v.len() > crate::key::MAX_KEY_BYTES {
        return Err(D::Error::custom("key bytes exceed MAX_KEY_BYTES"));
    }
    Ok(Bytes::from(v))
}

pub fn deserialize_value<'de, D>(deserializer: D) -> Result<Bytes, D::Error>
where
    D: Deserializer<'de>,
{
    let v = <Vec<u8>>::deserialize(deserializer)?;
    if v.len() > crate::value::MAX_VALUE_BYTES {
        return Err(D::Error::custom("value bytes exceed MAX_VALUE_BYTES"));
    }
    Ok(Bytes::from(v))
}
