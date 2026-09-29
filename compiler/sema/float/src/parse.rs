//! Decimal literals: digits, an optional fraction and an optional exponent, `_` between digits,
//! rounded exactly to a format.

use vun_sema_int::Integer;

use super::{Binary, Format};

impl Binary {
    /// The decimal `text` rounded to `format`; `None` for text that is not a decimal literal.
    #[must_use]
    pub fn parse(text: &str, format: Format) -> Option<Self> {
        let (number, exponent) = match text.split_once(['e', 'E']) {
            Some((number, exponent)) => (number, exponent.replace('_', "").parse::<i64>().ok()?),
            None => (text, 0),
        };
        let (whole, fraction) = number.split_once('.').unwrap_or((number, ""));
        let mut digits = Integer::zero();
        let (mut read, mut count) = (false, 0i64);
        for (part, fractional) in [(whole, false), (fraction, true)] {
            for c in part.chars().filter(|&c| c != '_') {
                let digit = c.to_digit(10)?;
                digits = &(&digits * &Integer::from(10)) + &Integer::from(i128::from(digit));
                read = true;
                count += i64::from(fractional);
            }
        }
        if !read {
            return None;
        }
        // `digits·10^power` is `digits·5^power·2^power`, or `digits/5^-power·2^power`.
        let power = exponent.checked_sub(count)?;
        if let Some(bound) = Self::beyond(format, &digits, power) {
            return Some(bound);
        }
        let five = Integer::from(5).pow(u32::try_from(power.unsigned_abs()).ok()?);
        let one = Integer::from(1);
        let (num, den) = if power >= 0 { (&digits * &five, one) } else { (digits, five) };
        Some(Self::round(format, false, &num, &den, power))
    }

    /// The zero, or the infinity, `digits·10^power` rounds to when it is zero or lies past
    /// `format`'s range, found from bounds on its binary exponent (`2^3 < 10 < 2^4`) so that no
    /// power of ten past the range is ever computed; `None` for a value that may lie within it.
    fn beyond(format: Format, digits: &Integer, power: i64) -> Option<Self> {
        if digits.is_zero() {
            return Some(Self::zero(format, false));
        }
        let bits = i64::try_from(digits.bits()).ok()?;
        // `2^(bits - 1) <= digits < 2^bits`, and `10^power` lies between `2^(3·power)` and
        // `2^(4·power)`, the lower power bounding from below when `power` is positive.
        let (three, four) = (power.saturating_mul(3), power.saturating_mul(4));
        let (low, high) = if power >= 0 { (three, four) } else { (four, three) };
        let top = format.ulp_max() + i64::from(format.precision());
        if (bits - 1).saturating_add(low) > top {
            return Some(Self::infinity(format, false));
        }
        (bits.saturating_add(high) < format.ulp_min() - 1).then(|| Self::zero(format, false))
    }
}
