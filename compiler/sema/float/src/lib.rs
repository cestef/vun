//! IEEE 754 binary floats at compile time (spec 04, section 2). A value is a format and its bits;
//! every operation computes its exact result and rounds it once, to nearest even, as the
//! hardware does at run time, so compile time and run time agree, for every format alike. A NaN
//! result is the format's canonical NaN, which lowering gives at run time too.

mod format;
mod ops;
mod parse;
mod round;
#[cfg(test)]
mod tests;

use std::fmt;

pub use format::Format;
use vun_codec::{Decode, Encode};
use vun_sema_int::Integer;

/// A float of `format`, by its bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Binary {
    pub format: Format,
    pub bits: u64,
}

/// What a float is: a NaN, an infinity, or a finite number `±mantissa·2^exponent` (zero among
/// them, with its sign).
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub(crate) enum Class {
    Nan,
    Infinite { negative: bool },
    Finite { negative: bool, mantissa: Integer, exponent: i64 },
}

impl Binary {
    /// The canonical NaN of `format`, every NaN result: sign clear, exponent all ones, only the
    /// top fraction bit set.
    #[must_use]
    pub fn nan(format: Format) -> Self {
        let bits = format.exponent_all() << format.fraction() | 1 << (format.fraction() - 1);
        Self { format, bits }
    }

    #[must_use]
    pub fn infinity(format: Format, negative: bool) -> Self {
        Self { format, bits: format.sign(negative) | format.exponent_all() << format.fraction() }
    }

    /// The finite float of `format` greatest in magnitude, with the sign `negative`.
    #[must_use]
    pub fn largest(format: Format, negative: bool) -> Self {
        Self { format, bits: Self::infinity(format, negative).bits - 1 }
    }

    #[must_use]
    pub fn zero(format: Format, negative: bool) -> Self {
        Self { format, bits: format.sign(negative) }
    }

    /// Whether its sign bit is set.
    #[must_use]
    pub fn negative(self) -> bool {
        self.bits & self.format.sign(true) != 0
    }

    pub(crate) fn class(self) -> Class {
        let fraction = self.format.fraction();
        let field = self.bits >> fraction & self.format.exponent_all();
        let low = self.bits & ((1 << fraction) - 1);
        let negative = self.negative();
        if field == self.format.exponent_all() {
            return if low == 0 { Class::Infinite { negative } } else { Class::Nan };
        }
        let (mantissa, exponent) = match field {
            0 => (low, self.format.ulp_min()),
            _ => (low | 1 << fraction, i64::try_from(field).expect("under 2^11") - 1 + self.format.ulp_min()),
        };
        Class::Finite { negative, mantissa: Integer::from(u128::from(mantissa)), exponent }
    }
}

impl fmt::Display for Binary {
    /// Its value as the host's `f64` shows it, which holds every value of every format exactly.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", f64::from_bits(self.widened().bits))
    }
}
