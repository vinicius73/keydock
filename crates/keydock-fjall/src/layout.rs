//! Keyspace names for bucket metadata vs key-value data.

/// Keyspace holding serialized [`BucketPolicy`](keydock_domain::BucketPolicy) per bucket id.
pub const META_KEYSPACE: &str = "meta";

/// Keyspace for key-value payloads (see [`crate::repos::data_storage_key`]).
pub const DATA_KEYSPACE: &str = "data";

/// Keyspace holding the expiry index for TTL garbage collection.
///
/// Keys are ordered by expiration timestamp so the sweeper can stop early.
pub const EXPIRY_KEYSPACE: &str = "expiry";
