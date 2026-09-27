//! Integer ranges and their arithmetic (spec 04, rules 1, 2, 5 to 7; whitepaper section 5): each
//! operation is monotone in each operand on the domains used, so its extremes are at the corners.

use super::{Extended, Integer};
use vun_codec::{Decode, Encode, Malformed, Reader};

/// The integers from `lo` to `hi`, both included: the values of `Int(lo..=hi)`, or of `Int(lo..)`
/// with an infinite end. Never empty.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode)]
pub struct Interval {
    lo: Extended,
    hi: Extended,
}

/// An interval read is one [`Interval::new`] makes.
impl Decode for Interval {
    fn decode(r: &mut Reader<'_>) -> Result<Self, Malformed> {
        let (lo, hi) = <(Extended, Extended)>::decode(r)?;
        Self::new(lo, hi).ok_or(Malformed::Invariant)
    }
}

/// Whether a divisor's range contains zero, which makes the division a trap site (rule 6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum Divisor {
    NonZero,
    MayBeZero,
}

/// The result of a division or remainder: its range over the divisors that are not zero, `None`
/// when the divisor is always zero, and whether it is a trap site.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct Divided {
    pub range: Option<Interval>,
    pub divisor: Divisor,
}

/// How a range is stored (rule 7): the smallest of these that holds it directly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub enum Storage {
    B8,
    B16,
    B32,
    B64,
    B128,
}

impl Storage {
    pub const ALL: [Self; 5] = [Self::B8, Self::B16, Self::B32, Self::B64, Self::B128];
    /// The widest storage: an integer needing more bits exists at compile time only (rule 5).
    pub const CAP: Self = Self::B128;

    #[must_use]
    pub const fn bits(self) -> u64 {
        match self {
            Self::B8 => 8,
            Self::B16 => 16,
            Self::B32 => 32,
            Self::B64 => 64,
            Self::B128 => 128,
        }
    }
}

impl Interval {
    /// `lo..=hi` over endpoints, or `None` when it holds nothing or starts at `+inf`.
    #[must_use]
    pub fn new(lo: Extended, hi: Extended) -> Option<Self> {
        (lo <= hi && lo != Extended::PosInf && hi != Extended::NegInf).then_some(Self { lo, hi })
    }

    /// `lo..=hi`, or `None` when it holds nothing.
    #[must_use]
    pub fn closed(lo: Integer, hi: Integer) -> Option<Self> {
        Self::new(Extended::Finite(lo), Extended::Finite(hi))
    }

    /// `lo..hi`, or `None` when it holds nothing.
    #[must_use]
    pub fn half_open(lo: Integer, hi: &Integer) -> Option<Self> {
        Self::closed(lo, hi - &Integer::from(1))
    }

    /// `lo..`: every integer from `lo` up.
    #[must_use]
    pub fn at_least(lo: Integer) -> Self {
        Self { lo: Extended::Finite(lo), hi: Extended::PosInf }
    }

    /// The type of a literal: exactly `n`.
    #[must_use]
    pub fn singleton(n: Integer) -> Self {
        Self { lo: Extended::Finite(n.clone()), hi: Extended::Finite(n) }
    }

    #[must_use]
    pub const fn lo(&self) -> &Extended {
        &self.lo
    }

    #[must_use]
    pub const fn hi(&self) -> &Extended {
        &self.hi
    }

    /// Whether every value of `other` is one of these: `Int(other)` is accepted where `Int(self)`
    /// is expected (rule 2).
    #[must_use]
    pub fn contains(&self, other: &Self) -> bool {
        self.lo <= other.lo && other.hi <= self.hi
    }

    /// The absolute values of these: `0..=5` for `-5..=3`.
    #[must_use]
    pub fn magnitude(&self) -> Self {
        let zero = Extended::Finite(Integer::from(0));
        if self.lo >= zero {
            return self.clone();
        }
        let negated = self.neg();
        if self.hi <= zero {
            return negated;
        }
        Self { lo: zero, hi: negated.hi.max(self.hi.clone()) }
    }

    /// The smallest range holding both, where two branches meet.
    #[must_use]
    pub fn hull(&self, other: &Self) -> Self {
        Self { lo: self.lo.clone().min(other.lo.clone()), hi: self.hi.clone().max(other.hi.clone()) }
    }

    /// The values in both, `None` when they share none.
    #[must_use]
    pub fn meet(&self, other: &Self) -> Option<Self> {
        Self::new(self.lo.clone().max(other.lo.clone()), self.hi.clone().min(other.hi.clone()))
    }

    /// The smallest range holding every endpoint.
    fn spanning(values: impl IntoIterator<Item = Extended>) -> Option<Self> {
        values.into_iter().fold(None, |range, n| {
            let point = Self { lo: n.clone(), hi: n };
            Some(range.map_or(point.clone(), |r: Self| r.hull(&point)))
        })
    }

    fn corners(&self, other: &Self, op: impl Fn(&Extended, &Extended) -> Extended) -> Self {
        let corners =
            [(&self.lo, &other.lo), (&self.lo, &other.hi), (&self.hi, &other.lo), (&self.hi, &other.hi)];
        Self::spanning(corners.map(|(a, b)| op(a, b))).expect("four corners")
    }

    #[must_use]
    pub fn add(&self, other: &Self) -> Self {
        Self { lo: self.lo.add(&other.lo), hi: self.hi.add(&other.hi) }
    }

    #[must_use]
    pub fn sub(&self, other: &Self) -> Self {
        self.add(&other.neg())
    }

    #[must_use]
    pub fn mul(&self, other: &Self) -> Self {
        self.corners(other, Extended::mul)
    }

    #[must_use]
    pub fn neg(&self) -> Self {
        Self { lo: self.hi.neg(), hi: self.lo.neg() }
    }

    /// The divisor's range without zero: its negative part and its positive part.
    fn nonzero_parts(&self) -> impl Iterator<Item = Self> {
        let (minus_one, one) = (Extended::Finite(Integer::from(-1)), Extended::Finite(Integer::from(1)));
        let below = Self::new(self.lo.clone(), self.hi.clone().min(minus_one));
        let above = Self::new(self.lo.clone().max(one), self.hi.clone());
        below.into_iter().chain(above)
    }

    fn divisor(&self) -> Divisor {
        let zero = Extended::Finite(Integer::zero());
        if self.lo <= zero && zero <= self.hi { Divisor::MayBeZero } else { Divisor::NonZero }
    }

    /// Division rounding toward zero: over each sign of the divisor, the quotient's extremes are
    /// at the corners.
    #[must_use]
    pub fn div(&self, divisor: &Self) -> Divided {
        let parts = divisor.nonzero_parts().map(|d| self.corners(&d, Extended::div));
        Divided { range: parts.reduce(|a, b| a.hull(&b)), divisor: divisor.divisor() }
    }

    /// The remainder of [`Self::div`]: its sign follows the dividend and its magnitude is below
    /// the divisor's, so it lies in `[max(min(lo, 0), 1 - m), min(max(hi, 0), m - 1)]` where `m`
    /// is the largest divisor magnitude.
    #[must_use]
    pub fn rem(&self, divisor: &Self) -> Divided {
        let magnitude = |e: &Extended| e.clone().max(e.neg());
        let largest = divisor.nonzero_parts().map(|d| magnitude(&d.lo).max(magnitude(&d.hi))).max();
        let range = largest.map(|m| {
            let (zero, one) = (Extended::Finite(Integer::zero()), Extended::Finite(Integer::from(1)));
            let lo = self.lo.clone().min(zero.clone()).max(one.add(&m.neg()));
            let hi = self.hi.clone().max(zero).min(m.add(&one.neg()));
            Self { lo, hi }
        });
        Divided { range, divisor: divisor.divisor() }
    }

    /// Whether a value can be negative, so that it is stored in two's complement.
    #[must_use]
    pub fn is_signed(&self) -> bool {
        self.lo < Extended::Finite(Integer::zero())
    }

    /// The fewest bits that hold every value directly, in two's complement when signed: the width
    /// of a `layout.bits` field (rule 7). `None` for an unbounded range.
    #[must_use]
    pub fn bits(&self) -> Option<u64> {
        let (lo, hi) = (self.lo.finite()?, self.hi.finite()?);
        Some(if self.is_signed() {
            let below = (&-lo - &Integer::from(1)).magnitude_bits();
            let above = if hi.is_negative() { 0 } else { hi.magnitude_bits() };
            below.max(above) + 1
        } else {
            hi.magnitude_bits()
        })
    }

    /// How many integers the range holds, when it is bounded.
    #[must_use]
    pub fn count(&self) -> Option<Integer> {
        let (lo, hi) = (self.lo.finite()?, self.hi.finite()?);
        Some(&(hi - lo) + &Integer::from(1))
    }

    /// Where the range is stored, or `None` past 128 bits (the width cap, rule 5) or for an
    /// unbounded range, which exists at compile time only.
    #[must_use]
    pub fn storage(&self) -> Option<Storage> {
        let bits = self.bits()?;
        Storage::ALL.into_iter().find(|s| bits <= s.bits())
    }
}
