use serde::{Deserialize, Serialize};

/// Identifier for a multi-key transaction (opaque).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TransactionId(pub String);
