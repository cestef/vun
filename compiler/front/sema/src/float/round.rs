//! Rounding an exact value to a format: to nearest, ties to even, into the subnormals below the
//! normal range and to an infinity above it.

use super::{Binary, Class, Format};
use crate::int::Integer;

impl Binary {
    /// `±num/den·2^scale`, `num` and `den` positive, rounded to `format`.
    pub(crate) fn round(format: Format, negative: bool, num: &Integer, den: &Integer, scale: i64) -> Self {
        if num.is_zero() {
            return Self::zero(format, negative);
        }
        let precision = i64::from(format.precision());
        let (low, high) =
            (Integer::power_of_two(format.fraction()), Integer::power_of_two(format.precision()));
        // `num/den·2^(scale - e)` as a numerator and a denominator.
        let scaled = |e: i64| {
            let shift = u32::try_from((scale - e).unsigned_abs()).expect("exponents of floats fit u32");
            if scale >= e {
                (num * &Integer::power_of_two(shift), den.clone())
            } else {
                (num.clone(), den * &Integer::power_of_two(shift))
            }
        };
        let quotient = |e: i64| {
            let (n, d) = scaled(e);
            n.checked_div(&d).expect("a positive denominator")
        };
        // The exponent of the last bit kept: the quotient has `precision` bits there.
        let bits = |n: &Integer| i64::try_from(n.magnitude_bits()).expect("fits i64");
        let mut e = scale + bits(num) - bits(den) - precision;
        loop {
            let q = quotient(e);
            if q >= high {
                e += 1;
            } else if q < low {
                e -= 1;
            } else {
                break;
            }
        }
        // Below the normal range, the last bit kept is the subnormals' least one.
        e = e.max(format.ulp_min());
        let (n, d) = scaled(e);
        let mut q = n.checked_div(&d).expect("a positive denominator");
        let twice = &(&n - &(&q * &d)) * &Integer::from(2);
        let odd = q.checked_rem(&Integer::from(2)).is_some_and(|r| !r.is_zero());
        if twice > d || (twice == d && odd) {
            q = &q + &Integer::from(1);
        }
        if q == high {
            q = low.clone();
            e += 1;
        }
        if e > format.ulp_max() {
            return Self::infinity(format, negative);
        }
        let field = if q < low { 0 } else { u64::try_from(e - format.ulp_min() + 1).expect("in range") };
        let q = u64::try_from(q.to_u128().expect("fits the fraction")).expect("fits the fraction");
        let fraction = q & ((1 << format.fraction()) - 1);
        Self { format, bits: format.sign(negative) | field << format.fraction() | fraction }
    }

    /// This value in `f64`, which holds every value of every format exactly.
    pub(crate) fn widened(self) -> Self {
        self.converted(Format::F64)
    }

    /// This value rounded to `format`; a NaN is its canonical NaN.
    #[must_use]
    pub fn converted(self, format: Format) -> Self {
        match self.class() {
            Class::Nan => Self::nan(format),
            Class::Infinite { negative } => Self::infinity(format, negative),
            Class::Finite { negative, mantissa, exponent } => {
                Self::round(format, negative, &mantissa, &Integer::from(1), exponent)
            }
        }
    }

    /// The float of `format` nearest `n`.
    #[must_use]
    pub fn of_integer(format: Format, n: &Integer) -> Self {
        Self::round(format, n.is_negative(), &n.abs(), &Integer::from(1), 0)
    }

    /// This value rounded toward zero, when it is finite.
    #[must_use]
    pub fn truncated(self) -> Option<Integer> {
        let Class::Finite { negative, mantissa, exponent } = self.class() else { return None };
        let shift = u32::try_from(exponent.unsigned_abs()).ok()?;
        let magnitude = if exponent >= 0 {
            &mantissa * &Integer::power_of_two(shift)
        } else {
            mantissa.checked_div(&Integer::power_of_two(shift))?
        };
        Some(if negative { -&magnitude } else { magnitude })
    }
}
