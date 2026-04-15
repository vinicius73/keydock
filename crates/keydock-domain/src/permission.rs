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
    pub const NONE: Self = Self {
        read: false,
        write: false,
        enumerate: false,
        delete: false,
    };

    pub const READ_ONLY: Self = Self {
        read: true,
        write: false,
        enumerate: false,
        delete: false,
    };

    pub const ADMIN: Self = Self {
        read: true,
        write: true,
        enumerate: true,
        delete: true,
    };
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn admin_has_all_flags() {
        let p = Permission::ADMIN;
        assert_eq!(p.read, true);
        assert_eq!(p.write, true);
        assert_eq!(p.enumerate, true);
        assert_eq!(p.delete, true);
    }

    #[test]
    fn read_only_has_only_read() {
        let p = Permission::READ_ONLY;
        assert_eq!(p.read, true);
        assert_eq!(p.write, false);
        assert_eq!(p.enumerate, false);
        assert_eq!(p.delete, false);
    }

    #[test]
    fn none_has_no_flags() {
        let p = Permission::NONE;
        assert_eq!(p.read, false);
        assert_eq!(p.write, false);
        assert_eq!(p.enumerate, false);
        assert_eq!(p.delete, false);
    }
}
