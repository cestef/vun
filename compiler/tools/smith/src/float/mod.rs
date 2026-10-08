//! The floats of the model (spec 04, section 4): a value of a format is held as the `f64` that
//! equals it, which every operation computes and rounds once to its format, the way the spec
//! says a float narrower than `f64` computes. A NaN an operation gives is the canonical one.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    reason = "the model converts between formats and compares by IEEE 754 on purpose"
)]

mod convert;
mod format;

pub use format::Format;

use crate::ir::{Arith, Compare};

/// A float: the bits of the `f64` equal to its value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Float(pub u64);

impl Float {
    #[must_use]
    pub fn of(value: f64) -> Self {
        Self(value.to_bits())
    }

    #[must_use]
    pub fn value(self) -> f64 {
        f64::from_bits(self.0)
    }

    /// The canonical NaN, which an operation that has none for a result gives.
    fn canonical(value: f64) -> f64 {
        if value.is_nan() { f64::NAN.copysign(1.0) } else { value }
    }

    /// `left op right` in `format`; `None` for an operation that is no float's.
    #[must_use]
    pub fn arith(format: Format, op: Arith, left: Self, right: Self) -> Option<Self> {
        let (a, b) = (left.value(), right.value());
        let value = match op {
            Arith::Add => a + b,
            Arith::Sub => a - b,
            Arith::Mul => a * b,
            Arith::Div => a / b,
            Arith::Rem => a % b,
            _ => return None,
        };
        Some(Self::of(Self::canonical(format.round(value))))
    }

    /// `-self`, which keeps every bit of a NaN but the sign.
    #[must_use]
    pub fn negated(self) -> Self {
        Self::of(-self.value())
    }

    /// Whether `left op right` holds: false of a NaN, but for `!=`.
    #[must_use]
    pub fn compare(op: Compare, left: Self, right: Self) -> bool {
        let (a, b) = (left.value(), right.value());
        match op {
            Compare::Eq => a == b,
            Compare::Ne => a != b,
            Compare::Lt => a < b,
            Compare::Le => a <= b,
            Compare::Gt => a > b,
            Compare::Ge => a >= b,
        }
    }
}
