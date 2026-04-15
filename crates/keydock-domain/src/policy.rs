use serde::{Deserialize, Serialize};

use crate::permission::Permission;

/// Policy fields attached to a bucket (subset of the public HTTP surface).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BucketPolicy {
    pub default_ttl_secs: Option<u64>,
    pub anonymous_access: Permission,
}
