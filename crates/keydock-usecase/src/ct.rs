//! Constant-time byte comparisons.

use subtle::ConstantTimeEq;

/// Compares two equal-length slices in constant time.
pub fn eq_bytes(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut acc = subtle::Choice::from(1u8);
    for (x, y) in a.iter().zip(b.iter()) {
        acc &= x.ct_eq(y);
    }
    acc.into()
}
