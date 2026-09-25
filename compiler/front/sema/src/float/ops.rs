//! IEEE 754 operations: each computes the exact result of finite operands and rounds it once;
//! NaNs, infinities and signed zeros follow the standard.

use std::cmp::Ordering;
use std::ops::{Add, Div, Mul, Neg, Rem, Sub};

use super::{Binary, Class};
use crate::int::Integer;

/// A finite value as a signed integer times a power of two.
struct Exact {
    value: Integer,
    exponent: i64,
}

impl Exact {
    /// Both at the lower exponent of the two.
    fn aligned(a: &Self, b: &Self) -> (Integer, Integer, i64) {
        let exponent = a.exponent.min(b.exponent);
        let scale = |x: &Self| {
            let shift = u32::try_from(x.exponent - exponent).unwrap_or(u32::MAX);
            &x.value * &Integer::power_of_two(shift)
        };
        (scale(a), scale(b), exponent)
    }
}

impl Binary {
    /// Its exact value, when it is finite.
    fn exact(self) -> Option<Exact> {
        let Class::Finite { negative, mantissa, exponent } = self.class() else { return None };
        Some(Exact { value: if negative { -&mantissa } else { mantissa }, exponent })
    }

    fn is_nan(self) -> bool {
        self.class() == Class::Nan
    }

    fn is_infinite(self) -> bool {
        matches!(self.class(), Class::Infinite { .. })
    }

    fn is_zero(self) -> bool {
        self.exact().is_some_and(|e| e.value.is_zero())
    }

    /// The exact value `value·2^exponent` rounded to this format; an exact zero is `+0`, as
    /// rounding to nearest gives it.
    fn of_exact(self, value: &Integer, exponent: i64) -> Self {
        Self::round(self.format, value.is_negative(), &value.abs(), &Integer::from(1), exponent)
    }

    /// `self·factor + addend`, rounded once.
    #[must_use]
    pub fn mul_add(self, factor: Self, addend: Self) -> Self {
        let product_negative = self.negative() != factor.negative();
        if self.is_nan() || factor.is_nan() || addend.is_nan() {
            return Self::nan(self.format);
        }
        if self.is_infinite() || factor.is_infinite() {
            let invalid = self.is_zero() || factor.is_zero();
            let clash = addend.is_infinite() && addend.negative() != product_negative;
            return if invalid || clash {
                Self::nan(self.format)
            } else {
                Self::infinity(self.format, product_negative)
            };
        }
        let (Some(first), Some(second), Some(third)) = (self.exact(), factor.exact(), addend.exact()) else {
            return addend;
        };
        let product =
            Exact { value: &first.value * &second.value, exponent: first.exponent + second.exponent };
        if product.value.is_zero() && third.value.is_zero() {
            return Self::zero(self.format, product_negative && addend.negative());
        }
        let (p, q, exponent) = Exact::aligned(&product, &third);
        self.of_exact(&(&p + &q), exponent)
    }

    /// Where it stands against `other`; `None` when either is a NaN, and zeros equal.
    #[must_use]
    pub fn compare(self, other: Self) -> Option<Ordering> {
        let rank = |x: Self| match x.class() {
            Class::Nan => None,
            Class::Infinite { negative } => Some(if negative { Ordering::Less } else { Ordering::Greater }),
            Class::Finite { .. } => Some(Ordering::Equal),
        };
        let (a, b) = (rank(self)?, rank(other)?);
        match (self.exact(), other.exact()) {
            (Some(x), Some(y)) => {
                let (x, y, _) = Exact::aligned(&x, &y);
                Some(x.cmp(&y))
            }
            _ => Some(a.cmp(&b)),
        }
    }

    /// IEEE `totalOrder`: `-NaN < -inf < ... < -0 < +0 < ... < inf < NaN`, by the bits with
    /// negative values' order reversed.
    #[must_use]
    pub fn total_order(self, other: Self) -> Ordering {
        let sign = self.format.sign(true);
        let mask = sign | (sign - 1);
        let key = |x: Self| if x.bits & sign != 0 { !x.bits & mask } else { x.bits | sign };
        key(self).cmp(&key(other))
    }
}

impl Neg for Binary {
    type Output = Self;
    /// The same value with the other sign: the sign bit flipped, a NaN's too.
    fn neg(self) -> Self {
        Self { bits: self.bits ^ self.format.sign(true), ..self }
    }
}

impl Add for Binary {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        match (self.exact(), other.exact()) {
            _ if self.is_nan() || other.is_nan() => Self::nan(self.format),
            _ if self.is_infinite() && other.is_infinite() && self.negative() != other.negative() => {
                Self::nan(self.format)
            }
            _ if self.is_infinite() => self,
            _ if other.is_infinite() => other,
            _ if self.is_zero() && other.is_zero() => {
                Self::zero(self.format, self.negative() && other.negative())
            }
            _ if other.is_zero() => self,
            _ if self.is_zero() => other,
            (Some(a), Some(b)) => {
                let (a, b, exponent) = Exact::aligned(&a, &b);
                self.of_exact(&(&a + &b), exponent)
            }
            _ => Self::nan(self.format),
        }
    }
}

impl Sub for Binary {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        self + -other
    }
}

impl Mul for Binary {
    type Output = Self;
    fn mul(self, other: Self) -> Self {
        let negative = self.negative() != other.negative();
        if self.is_nan() || other.is_nan() {
            return Self::nan(self.format);
        }
        if self.is_infinite() || other.is_infinite() {
            let invalid = self.is_zero() || other.is_zero();
            return if invalid { Self::nan(self.format) } else { Self::infinity(self.format, negative) };
        }
        let (Some(a), Some(b)) = (self.exact(), other.exact()) else { return Self::nan(self.format) };
        let product = &a.value * &b.value;
        if product.is_zero() {
            return Self::zero(self.format, negative);
        }
        self.of_exact(&product, a.exponent + b.exponent)
    }
}

impl Div for Binary {
    type Output = Self;
    fn div(self, other: Self) -> Self {
        let negative = self.negative() != other.negative();
        match (self.exact(), other.exact()) {
            _ if self.is_nan() || other.is_nan() => Self::nan(self.format),
            _ if self.is_infinite() && other.is_infinite() => Self::nan(self.format),
            _ if self.is_infinite() => Self::infinity(self.format, negative),
            _ if other.is_infinite() => Self::zero(self.format, negative),
            _ if other.is_zero() && self.is_zero() => Self::nan(self.format),
            _ if other.is_zero() => Self::infinity(self.format, negative),
            _ if self.is_zero() => Self::zero(self.format, negative),
            (Some(a), Some(b)) => {
                Self::round(self.format, negative, &a.value.abs(), &b.value.abs(), a.exponent - b.exponent)
            }
            _ => Self::nan(self.format),
        }
    }
}

/// The remainder of the division truncated toward zero, `x - trunc(x / y)·y`, which is exact:
/// C's `fmod`.
impl Rem for Binary {
    type Output = Self;
    fn rem(self, other: Self) -> Self {
        match (self.exact(), other.exact()) {
            _ if self.is_nan() || other.is_nan() || self.is_infinite() || other.is_zero() => {
                Self::nan(self.format)
            }
            _ if other.is_infinite() || self.is_zero() => self,
            (Some(a), Some(b)) => {
                let (a, b, exponent) = Exact::aligned(&a, &b);
                match a.abs().checked_rem(&b.abs()) {
                    Some(r) if !r.is_zero() => {
                        Self::round(self.format, self.negative(), &r, &Integer::from(1), exponent)
                    }
                    _ => Self::zero(self.format, self.negative()),
                }
            }
            _ => Self::nan(self.format),
        }
    }
}
