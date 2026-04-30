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

    /// Read plus enumerate: identity derived from the bucket `read_key`
    /// (listing is part of the read side).
    pub const READ_ENUMERATE: Self = Self::new(true, false, true, false);

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
    use rstest::rstest;

    use super::*;

    #[test]
    fn admin_has_all_flags() {
        assert_eq!(Permission::ADMIN, Permission::new(true, true, true, true));
    }

    #[test]
    fn read_only_has_only_read() {
        assert_eq!(
            Permission::READ_ONLY,
            Permission::new(true, false, false, false)
        );
    }

    #[test]
    fn none_has_no_flags() {
        assert_eq!(
            Permission::NONE,
            Permission::new(false, false, false, false)
        );
    }

    #[test]
    fn union_combines_flags() {
        let a = Permission::new(true, false, false, false);
        let b = Permission::new(false, true, false, true);
        assert_eq!(a.union(b), Permission::new(true, true, false, true));
    }

    #[rstest]
    #[case::no_keys(false, false, false, Permission::new(true, true, true, true))]
    #[case::secret_only(true, false, false, Permission::new(true, true, true, false))]
    #[case::read_only(false, true, false, Permission::new(false, true, false, false))]
    #[case::write_only(false, false, true, Permission::new(true, false, true, false))]
    #[case::secret_and_read(true, true, false, Permission::new(false, true, false, false))]
    #[case::secret_and_write(true, false, true, Permission::new(true, false, true, false))]
    #[case::read_and_write(false, true, true, Permission::new(false, false, false, false))]
    #[case::all_three(true, true, true, Permission::new(false, false, false, false))]
    fn anonymous_from_keys_matrix(
        #[case] has_secret: bool,
        #[case] has_read: bool,
        #[case] has_write: bool,
        #[case] expected: Permission,
    ) {
        assert_eq!(
            Permission::anonymous_from_keys(has_secret, has_read, has_write),
            expected
        );
    }
}
