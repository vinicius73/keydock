//! Keyspace names for bucket metadata vs key-value data.

/// Keyspace holding serialized [`BucketPolicy`](keydock_domain::BucketPolicy) per bucket id.
pub const META_KEYSPACE: &str = "meta";

/// Keyspace for key-value payloads (future use).
pub const DATA_KEYSPACE: &str = "data";
