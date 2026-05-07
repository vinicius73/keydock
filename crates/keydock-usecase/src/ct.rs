//! Constant-time byte comparisons.

use subtle::ConstantTimeEq;

/// Compares two byte slices in constant time over their equal-length content.
///
/// Length mismatch is checked up front (the length itself is not secret here —
/// it's a function of the stored hash size known to both sides); over equal-length
/// inputs, [`ConstantTimeEq`] guarantees no early exit on the first differing byte.
pub fn eq_bytes(a: &[u8], b: &[u8]) -> bool {
    a.ct_eq(b).into()
}
