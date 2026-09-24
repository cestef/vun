//! Exact integers (spec 04, section 1): compile-time values have no bound, so every value and
//! every range bound is an arbitrary-precision integer.

mod extended;
mod interval;
#[cfg(test)]
mod tests;

use std::fmt;
use std::ops::{Add, Mul, Neg, Sub};

use num_bigint::{BigInt, Sign};

pub use extended::Extended;
pub use interval::{Divided, Divisor, Interval, Storage};

/// An integer of any size.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Integer(BigInt);

impl Integer {
    #[must_use]
    pub fn zero() -> Self {
        Self::from(0)
    }

    #[must_use]
    pub fn is_negative(&self) -> bool {
        self.0.sign() == Sign::Minus
    }

    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.0.sign() == Sign::NoSign
    }

    #[must_use]
    pub fn abs(&self) -> Self {
        Self(BigInt::from_biguint(Sign::Plus, self.0.magnitude().clone()))
    }

    /// `2^exponent`.
    #[must_use]
    pub fn power_of_two(exponent: u32) -> Self {
        Self(BigInt::from(1) << exponent)
    }

    /// The quotient rounded toward zero, or `None` for a zero divisor.
    #[must_use]
    pub fn checked_div(&self, divisor: &Self) -> Option<Self> {
        (!divisor.is_zero()).then(|| Self(&self.0 / &divisor.0))
    }

    /// The remainder of [`Self::checked_div`], whose sign follows the dividend.
    #[must_use]
    pub fn checked_rem(&self, divisor: &Self) -> Option<Self> {
        (!divisor.is_zero()).then(|| Self(&self.0 % &divisor.0))
    }

    /// The remainder of dividing by `modulus` that has the modulus's sign, as modular reduction
    /// computes it; `None` for a zero modulus.
    #[must_use]
    pub fn modulo(&self, modulus: &Self) -> Option<Self> {
        (!modulus.is_zero()).then(|| Self(((&self.0 % &modulus.0) + &modulus.0) % &modulus.0))
    }

    /// `self * 2^amount`, or `self / 2^amount` rounded toward negative infinity for a negative
    /// amount, as `<<` and `>>` compute at compile time; `None` for an amount past `u32`.
    #[must_use]
    pub fn shifted(&self, amount: &Self) -> Option<Self> {
        let by = u32::try_from(&amount.abs().0).ok()?;
        Some(Self(if amount.is_negative() { &self.0 >> by } else { &self.0 << by }))
    }

    /// How many bits the magnitude needs: 0 for zero.
    #[must_use]
    pub fn magnitude_bits(&self) -> u64 {
        self.0.magnitude().bits()
    }

    /// The value, if it fits `i128`.
    #[must_use]
    pub fn to_i128(&self) -> Option<i128> {
        i128::try_from(&self.0).ok()
    }
}

impl From<i128> for Integer {
    fn from(n: i128) -> Self {
        Self(BigInt::from(n))
    }
}

impl From<u128> for Integer {
    fn from(n: u128) -> Self {
        Self(BigInt::from(n))
    }
}

impl From<usize> for Integer {
    fn from(n: usize) -> Self {
        Self(BigInt::from(n))
    }
}

impl From<i32> for Integer {
    fn from(n: i32) -> Self {
        Self(BigInt::from(n))
    }
}

impl fmt::Display for Integer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Operators on references, the form interval arithmetic uses.
macro_rules! binary {
    ($($op:ident $method:ident),*) => {$(
        impl $op for &Integer {
            type Output = Integer;
            fn $method(self, other: Self) -> Integer {
                Integer((&self.0).$method(&other.0))
            }
        }
    )*};
}
binary!(Add add, Sub sub, Mul mul);

impl Neg for &Integer {
    type Output = Integer;
    fn neg(self) -> Integer {
        Integer(-&self.0)
    }
}
