use serde::{Deserialize, Serialize};

/// Coarse permission flags for temporary tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Permission {
    pub read: bool,
    pub write: bool,
    pub delete: bool,
}

impl Permission {
    pub const READ_ONLY: Self = Self {
        read: true,
        write: false,
        delete: false,
    };
}
