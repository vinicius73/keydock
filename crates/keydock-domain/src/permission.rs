use serde::{Deserialize, Serialize};

/// Coarse permission flags for temporary tokens and bucket anonymous access.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Permission {
    pub read: bool,
    pub write: bool,
    pub enumerate: bool,
    pub delete: bool,
}

impl Permission {
    pub const NONE: Self = Self::new(false, false, false, false);

    pub const READ_ONLY: Self = Self::new(true, false, false, false);

    pub const WRITE_ONLY: Self = Self::new(false, true, false, false);

    pub const ADMIN: Self = Self::new(true, true, true, true);

    /// Constructs permission flags for temporary tokens and anonymous bucket access.
    #[must_use]
    pub const fn new(read: bool, write: bool, enumerate: bool, delete: bool) -> Self {
        Self {
            read,
            write,
            enumerate,
            delete,
        }
    }

    /// Derives anonymous-access flags from the presence of per-capability API key hashes.
    ///
    /// A capability is anonymously granted when the matching key is **absent** (no credential
    /// is required to use it). `delete` additionally requires that no administrative key
    /// (secret/read/write) is configured.
    #[must_use]
    pub const fn anonymous_from_keys(has_secret: bool, has_read: bool, has_write: bool) -> Self {
        Self {
            read: !has_read,
            write: !has_write,
            enumerate: !has_read,
            delete: !has_secret && !has_read && !has_write,
        }
    }

    /// Returns the union of capability bits (OR of each flag).
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self {
            read: self.read || other.read,
            write: self.write || other.write,
            enumerate: self.enumerate || other.enumerate,
            delete: self.delete || other.delete,
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn admin_has_all_flags() {
        let p = Permission::ADMIN;
        assert!(p.read);
        assert!(p.write);
        assert!(p.enumerate);
        assert!(p.delete);
    }

    #[test]
    fn read_only_has_only_read() {
        let p = Permission::READ_ONLY;
        assert!(p.read);
        assert!(!p.write);
        assert!(!p.enumerate);
        assert!(!p.delete);
    }

    #[test]
    fn none_has_no_flags() {
        let p = Permission::NONE;
        assert!(!p.read);
        assert!(!p.write);
        assert!(!p.enumerate);
        assert!(!p.delete);
    }

    #[test]
    fn union_combines_flags() {
        let a = Permission::new(true, false, false, false);
        let b = Permission::new(false, true, false, true);
        assert_eq!(a.union(b), Permission::new(true, true, false, true));
    }
}
