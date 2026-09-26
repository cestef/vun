//! Bit operations (spec 04, rule 8): on a `uN` range they keep the range, a constant meeting one
//! taking its type, and on compile-time values, whose ranges hold one integer, they are exact.

use num_bigint::BigInt;
use vun_codec::{Decode, Encode};

use super::{Extended, Integer, Interval};

/// `&`, `|` or `^`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum Bitwise {
    And,
    Or,
    Xor,
}

impl Integer {
    /// `self op other`, in two's complement.
    #[must_use]
    pub fn bitwise(&self, other: &Self, op: Bitwise) -> Self {
        Self(match op {
            Bitwise::And => &self.0 & &other.0,
            Bitwise::Or => &self.0 | &other.0,
            Bitwise::Xor => &self.0 ^ &other.0,
        })
    }

    /// Every bit of the `bits` lowest flipped: `~x` in `unsigned(bits)`.
    #[must_use]
    pub fn complement(&self, bits: u32) -> Self {
        Self((BigInt::from(1) << bits) - 1 - &self.0)
    }
}

impl Interval {
    /// `N` when this is the range of `unsigned(N)`, `0..2^N`.
    #[must_use]
    pub fn unsigned_width(&self) -> Option<u32> {
        let bits = u32::try_from(self.bits()?).ok()?;
        (self.is_unsigned_width() && bits > 0).then_some(bits)
    }

    /// The one integer this range holds, the type of a compile-time value.
    #[must_use]
    pub fn value(&self) -> Option<&Integer> {
        let (lo, hi) = (self.lo().finite()?, self.hi().finite()?);
        (lo == hi).then_some(lo)
    }

    /// The range of a bit operation's result: the `uN` range of one operand when the other lies
    /// within it, or, for two non-negative compile-time values, the smallest `uN` range holding
    /// both; `None` for operands of no `uN` type.
    #[must_use]
    pub fn bitwise(&self, other: &Self) -> Option<Self> {
        if self.unsigned_width().is_some() && self.contains(other) {
            return Some(self.clone());
        }
        if other.unsigned_width().is_some() && other.contains(self) {
            return Some(other.clone());
        }
        let (a, b) = (self.value()?, other.value()?);
        if a.is_negative() || b.is_negative() {
            return None;
        }
        let bits = u32::try_from(a.magnitude_bits().max(b.magnitude_bits()).max(1)).ok()?;
        Self::half_open(Integer::zero(), &Integer::power_of_two(bits))
    }

    /// The range of `self << amount`, or `>>` for `right`: a `uN` range keeps itself; a
    /// compile-time value shifted by any amount of a bounded range spans the shifts of its ends.
    /// `None` for anything else.
    #[must_use]
    pub fn shifted(&self, amount: &Self, right: bool) -> Option<Self> {
        if self.unsigned_width().is_some() {
            return Some(self.clone());
        }
        let value = self.value()?;
        let (lo, hi) = (amount.lo().finite()?, amount.hi().finite()?);
        let by = |n: &Integer| if right { value.shifted(&(&Integer::zero() - n)) } else { value.shifted(n) };
        let ends = [by(lo)?, by(hi)?];
        let (low, high) = if ends[0] <= ends[1] { (&ends[0], &ends[1]) } else { (&ends[1], &ends[0]) };
        Self::new(Extended::Finite(low.clone()), Extended::Finite(high.clone()))
    }
}
