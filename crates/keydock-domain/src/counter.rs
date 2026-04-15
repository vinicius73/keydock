//! Counter delta parsing and numeric promotion for atomic increments.

use bytes::Bytes;

use crate::value::ValueKind;
use crate::{DomainError, StoredValue};

/// Parsed counter delta from a `PATCH` body (`+N` / `-N`, integer or float).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CounterOp {
    Int(i64),
    Float(f64),
}

/// Current numeric value used when applying a counter delta.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CounterValue {
    Int(i64),
    Float(f64),
}

impl CounterOp {
    /// Parses a counter operation from raw body bytes.
    ///
    /// Grammar: optional surrounding ASCII whitespace + mandatory `+` or `-` + `i64` or `f64` literal.
    pub fn parse(body: &[u8]) -> Result<Self, DomainError> {
        let s = std::str::from_utf8(body)
            .map_err(|_| DomainError::InvalidCounterOp("body is not valid utf-8".into()))?;
        let t = s.trim();
        if t.is_empty() {
            return Err(DomainError::InvalidCounterOp("empty body".into()));
        }
        let mut chars = t.chars();
        let Some(first) = chars.next() else {
            return Err(DomainError::InvalidCounterOp("empty body".into()));
        };
        if first != '+' && first != '-' {
            return Err(DomainError::InvalidCounterOp("missing +/- prefix".into()));
        }

        if let Ok(i) = t.parse::<i64>() {
            return Ok(CounterOp::Int(i));
        }
        if let Ok(f) = t.parse::<f64>() {
            if f.is_nan() || f.is_infinite() {
                return Err(DomainError::InvalidCounterOp("invalid float".into()));
            }
            return Ok(CounterOp::Float(f));
        }
        Err(DomainError::InvalidCounterOp("invalid number".into()))
    }

    /// Applies this delta to `current`, promoting to float when any operand is float.
    ///
    /// Integer overflow on `Int + Int` returns [`DomainError::InvalidCounterOp`].
    pub fn apply(self, current: CounterValue) -> Result<CounterValue, DomainError> {
        match (self, current) {
            (CounterOp::Int(delta), CounterValue::Int(v)) => v
                .checked_add(delta)
                .map(CounterValue::Int)
                .ok_or_else(|| DomainError::InvalidCounterOp("integer overflow".into())),
            (CounterOp::Int(delta), CounterValue::Float(v)) => {
                Self::float_result(v + (delta as f64))
            }
            (CounterOp::Float(delta), CounterValue::Int(v)) => {
                Self::float_result((v as f64) + delta)
            }
            (CounterOp::Float(delta), CounterValue::Float(v)) => Self::float_result(v + delta),
        }
    }

    fn float_result(nf: f64) -> Result<CounterValue, DomainError> {
        if nf.is_nan() || nf.is_infinite() {
            return Err(DomainError::InvalidCounterOp("invalid result".into()));
        }
        Ok(CounterValue::Float(nf))
    }
}

impl CounterValue {
    /// Interprets a stored value as a counter operand.
    ///
    /// Only [`ValueKind::Int64`] and [`ValueKind::Float64`] are accepted.
    pub fn from_stored(value: &StoredValue) -> Result<Self, DomainError> {
        match value.kind {
            ValueKind::Int64 => {
                let s = std::str::from_utf8(value.payload.as_ref()).map_err(|_| {
                    DomainError::InvalidCounterOp("stored int is not valid utf-8".into())
                })?;
                let i: i64 = s.trim().parse().map_err(|_| {
                    DomainError::InvalidCounterOp("stored int is not parseable".into())
                })?;
                Ok(CounterValue::Int(i))
            }
            ValueKind::Float64 => {
                let s = std::str::from_utf8(value.payload.as_ref()).map_err(|_| {
                    DomainError::InvalidCounterOp("stored float is not valid utf-8".into())
                })?;
                let f: f64 = s.trim().parse().map_err(|_| {
                    DomainError::InvalidCounterOp("stored float is not parseable".into())
                })?;
                if f.is_nan() || f.is_infinite() {
                    return Err(DomainError::InvalidCounterOp(
                        "stored float is not finite".into(),
                    ));
                }
                Ok(CounterValue::Float(f))
            }
            _ => Err(DomainError::InvalidCounterOp("value is not numeric".into())),
        }
    }

    /// Serializes this counter value for storage.
    ///
    /// Counter results always fit within [`crate::value::MAX_VALUE_BYTES`].
    pub fn into_stored(self) -> Result<StoredValue, DomainError> {
        match self {
            CounterValue::Int(i) => {
                StoredValue::new(Bytes::from(format!("{i}").into_bytes()), ValueKind::Int64)
            }
            CounterValue::Float(f) => {
                StoredValue::new(Bytes::from(format!("{f}").into_bytes()), ValueKind::Float64)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use crate::StoredValue;
    use crate::value::ValueKind;

    use super::*;

    #[rstest]
    #[case::plus_one(b"+1", CounterOp::Int(1))]
    #[case::minus_three(b"-3", CounterOp::Int(-3))]
    #[case::plus_float(b"+1.5", CounterOp::Float(1.5))]
    #[case::whitespace(b"  +2  ", CounterOp::Int(2))]
    fn parse_ok(#[case] body: &[u8], #[case] expected: CounterOp) {
        assert_eq!(CounterOp::parse(body).expect("parse"), expected);
    }

    #[rstest]
    #[case::no_sign(b"5")]
    #[case::empty(b"")]
    #[case::only_plus(b"+")]
    #[case::nan(b"+NaN")]
    #[case::inf(b"+Inf")]
    fn parse_err(#[case] body: &[u8]) {
        assert_eq!(CounterOp::parse(body).is_err(), true);
    }

    #[test]
    fn apply_int_plus_int_checked() {
        let v = CounterOp::Int(3)
            .apply(CounterValue::Int(10))
            .expect("apply");
        assert_eq!(v, CounterValue::Int(13));
    }

    #[test]
    fn apply_int_overflow_err() {
        let err = CounterOp::Int(1)
            .apply(CounterValue::Int(i64::MAX))
            .expect_err("overflow");
        assert_eq!(matches!(err, DomainError::InvalidCounterOp(_)), true);
    }

    #[test]
    fn apply_promotes_int_to_float() {
        let v = CounterOp::Float(1.5)
            .apply(CounterValue::Int(10))
            .expect("apply");
        assert_eq!(v, CounterValue::Float(11.5));
    }

    #[test]
    fn apply_float_plus_int() {
        let v = CounterOp::Int(1)
            .apply(CounterValue::Float(1.5))
            .expect("apply");
        assert_eq!(v, CounterValue::Float(2.5));
    }

    #[test]
    fn from_stored_rejects_utf8_string() {
        let sv = StoredValue::new(Bytes::from_static(b"hello"), ValueKind::Utf8).expect("value");
        let err = CounterValue::from_stored(&sv).expect_err("non-numeric");
        assert_eq!(matches!(err, DomainError::InvalidCounterOp(_)), true);
    }

    #[test]
    fn from_stored_accepts_int64() {
        let sv = StoredValue::new(Bytes::from_static(b"42"), ValueKind::Int64).expect("value");
        assert_eq!(
            CounterValue::from_stored(&sv).expect("ok"),
            CounterValue::Int(42)
        );
    }

    #[test]
    fn roundtrip_into_stored() {
        let v = CounterValue::Int(-7);
        let sv = v.into_stored().expect("stored");
        assert_eq!(CounterValue::from_stored(&sv).expect("round"), v);
    }
}
