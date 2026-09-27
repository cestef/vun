//! Exact integers (spec 04, section 1): compile-time values have no bound, so every value and
//! every range bound is an arbitrary-precision integer.

mod bits;
mod compare;
mod extended;
mod interval;
#[cfg(test)]
mod tests;

use std::fmt;
use std::ops::{Add, Mul, Neg, Sub};

use num_bigint::{BigInt, Sign};
use vun_codec::{Decode, Encode, Malformed, Reader, Writer};

pub use bits::{Bitwise, Shift};
pub use compare::Compared;
pub use extended::Extended;
pub use interval::{Divided, Divisor, Interval, Storage};

/// The most bits a shift at compile time makes a value grow to: a shift past it traps
/// ([`crate::trap::Reason::Shift`]) rather than allocate without bound.
pub const MAX_BITS: u64 = 1 << 24;

/// An integer of any size.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Integer(BigInt);

/// Its two's complement bytes, least significant first.
impl Encode for Integer {
    fn encode(&self, w: &mut Writer<'_>) {
        self.0.to_signed_bytes_le().encode(w);
    }
}

impl Decode for Integer {
    fn decode(r: &mut Reader<'_>) -> Result<Self, Malformed> {
        Vec::<u8>::decode(r).map(|bytes| Self(BigInt::from_signed_bytes_le(&bytes)))
    }
}

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

    /// `self^exponent`.
    #[must_use]
    pub fn pow(&self, exponent: u32) -> Self {
        Self(self.0.pow(exponent))
    }

    /// The number of bits of the magnitude: 0 for zero, `k + 1` for `2^k`.
    #[must_use]
    pub fn bits(&self) -> u64 {
        self.0.magnitude().bits()
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

    /// `self << amount` (`self * 2^amount`) or `self >> amount` (`self / 2^amount` rounded
    /// toward negative infinity), as compile time computes them; a negative amount shifts the
    /// other way. `None` for a result past [`MAX_BITS`].
    #[must_use]
    pub fn shifted(&self, shift: Shift, amount: &Self) -> Option<Self> {
        let by = u64::try_from(&amount.abs().0).unwrap_or(u64::MAX);
        if (shift == Shift::Left) == amount.is_negative() {
            // Past the magnitude's bits every result is 0, or -1 for a negative value.
            return Some(Self(&self.0 >> by.min(self.bits())));
        }
        if self.is_zero() {
            return Some(Self::zero());
        }
        self.bits().checked_add(by).filter(|bits| *bits <= MAX_BITS).map(|_| Self(&self.0 << by))
    }

    /// How many bits the magnitude needs: 0 for zero.
    #[must_use]
    pub fn magnitude_bits(&self) -> u64 {
        self.0.magnitude().bits()
    }

    /// The value, if it fits `u128`.
    #[must_use]
    pub fn to_u128(&self) -> Option<u128> {
        u128::try_from(&self.0).ok()
    }

    /// The value, if it fits `i128`.
    #[must_use]
    pub fn to_i128(&self) -> Option<i128> {
        i128::try_from(&self.0).ok()
    }
}

impl From<BigInt> for Integer {
    fn from(n: BigInt) -> Self {
        Self(n)
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
