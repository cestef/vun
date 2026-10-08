//! Floats to integers, integers to floats, and floats to floats (spec 04, section 4).

use super::{Float, Format};
use crate::int::{BigInt, Range};

impl Float {
    /// The float of `format` nearest the integer `n`, ties to even, rounded once.
    ///
    /// # Panics
    /// If `n` is wider than 128 bits.
    #[must_use]
    pub fn from_int(format: Format, n: &BigInt) -> Self {
        let magnitude = n.magnitude();
        let length = magnitude.bits();
        let precision = u64::from(format.precision());
        let value = if length <= precision {
            u128::try_from(magnitude).expect("a short integer fits 128 bits") as f64
        } else {
            let shift = length - precision;
            let kept: u128 = u128::try_from(magnitude >> shift).expect("a short integer fits 128 bits");
            let dropped = magnitude - ((magnitude >> shift) << shift);
            let half = num_bigint::BigUint::from(1u8) << (shift - 1);
            let up = dropped > half || (dropped == half && kept % 2 == 1);
            (kept + u128::from(up)) as f64 * 2f64.powi(i32::try_from(shift).expect("at most 128"))
        };
        let signed = if n.sign() == num_bigint::Sign::Minus { -value } else { value };
        Self::of(format.round(signed))
    }

    /// The float of `to` nearest `self`, a float of `from`; a NaN gives the canonical one.
    #[must_use]
    pub fn converted(self, to: Format) -> Self {
        Self::of(Self::canonical(to.round(self.value())))
    }

    /// `self` rounded toward zero and clamped to `range`; a NaN gives 0, clamped likewise.
    #[must_use]
    pub fn truncated(self, range: &Range) -> BigInt {
        let value = self.value();
        if value.is_nan() {
            return range.saturate(&BigInt::ZERO);
        }
        if value.is_infinite() {
            return if value > 0.0 { range.hi.clone() } else { range.lo.clone() };
        }
        let whole = value.trunc();
        let bits = whole.to_bits();
        let exponent = ((bits >> 52) & 0x7ff) as i32 - 1075;
        let significand = if whole == 0.0 { 0 } else { (bits & ((1 << 52) - 1)) | (1 << 52) };
        let magnitude = if exponent >= 0 {
            BigInt::from(significand) << exponent.unsigned_abs()
        } else {
            BigInt::from(significand >> exponent.unsigned_abs().min(63))
        };
        range.saturate(&if whole < 0.0 { -magnitude } else { magnitude })
    }
}
