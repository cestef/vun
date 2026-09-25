//! Decimal literals: digits, an optional fraction and an optional exponent, `_` between digits,
//! rounded exactly to a format.

use super::{Binary, Format};
use crate::int::Integer;

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
        let mut count = 0i64;
        for (part, fractional) in [(whole, false), (fraction, true)] {
            for c in part.chars().filter(|&c| c != '_') {
                let digit = c.to_digit(10)?;
                digits = &(&digits * &Integer::from(10)) + &Integer::from(i128::from(digit));
                count += i64::from(fractional);
            }
        }
        // `digits·10^power` is `digits·5^power·2^power`, or `digits/5^-power·2^power`.
        let power = exponent - count;
        let five = Integer::from(5).pow(u32::try_from(power.unsigned_abs()).ok()?);
        let one = Integer::from(1);
        let (num, den) = if power >= 0 { (&digits * &five, one) } else { (digits, five) };
        Some(Self::round(format, false, &num, &den, power))
    }
}
