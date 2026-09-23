//! Integer ranges and their arithmetic (spec 04, rules 1, 2, 5 to 7; whitepaper section 5): each
//! operation is monotone in each operand on the domains used, so its extremes are at the corners.

use super::Integer;

/// The integers from `lo` to `hi`, both included: the values of `Int(lo..=hi)`. Never empty.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Interval {
    lo: Integer,
    hi: Integer,
}

/// Whether a divisor's range contains zero, which makes the division a trap site (rule 6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Divisor {
    NonZero,
    MayBeZero,
}

/// The result of a division or remainder: its range over the divisors that are not zero, `None`
/// when the divisor is always zero, and whether it is a trap site.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Divided {
    pub range: Option<Interval>,
    pub divisor: Divisor,
}

/// How a range is stored (rule 7): the smallest of these that holds it directly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Storage {
    B8,
    B16,
    B32,
    B64,
    B128,
}

impl Storage {
    pub const ALL: [Self; 5] = [Self::B8, Self::B16, Self::B32, Self::B64, Self::B128];

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
    /// `lo..=hi`, or `None` when it holds nothing.
    #[must_use]
    pub fn closed(lo: Integer, hi: Integer) -> Option<Self> {
        (lo <= hi).then_some(Self { lo, hi })
    }

    /// `lo..hi`, or `None` when it holds nothing.
    #[must_use]
    pub fn half_open(lo: Integer, hi: &Integer) -> Option<Self> {
        Self::closed(lo, hi - &Integer::from(1))
    }

    /// The type of a literal: exactly `n`.
    #[must_use]
    pub fn singleton(n: Integer) -> Self {
        Self { lo: n.clone(), hi: n }
    }

    #[must_use]
    pub const fn lo(&self) -> &Integer {
        &self.lo
    }

    #[must_use]
    pub const fn hi(&self) -> &Integer {
        &self.hi
    }

    /// Whether every value of `other` is one of these: `Int(other)` is accepted where `Int(self)`
    /// is expected (rule 2).
    #[must_use]
    pub fn contains(&self, other: &Self) -> bool {
        self.lo <= other.lo && other.hi <= self.hi
    }

    /// The smallest range holding both, where two branches meet.
    #[must_use]
    pub fn hull(&self, other: &Self) -> Self {
        Self { lo: self.lo.clone().min(other.lo.clone()), hi: self.hi.clone().max(other.hi.clone()) }
    }

    /// The smallest range holding every value.
    fn spanning(values: impl IntoIterator<Item = Integer>) -> Option<Self> {
        values.into_iter().fold(None, |range, n| {
            let point = Self::singleton(n);
            Some(range.map_or(point.clone(), |r: Self| r.hull(&point)))
        })
    }

    fn corners(&self, other: &Self, op: impl Fn(&Integer, &Integer) -> Integer) -> Self {
        let corners =
            [(&self.lo, &other.lo), (&self.lo, &other.hi), (&self.hi, &other.lo), (&self.hi, &other.hi)];
        Self::spanning(corners.map(|(a, b)| op(a, b))).expect("four corners")
    }

    #[must_use]
    pub fn add(&self, other: &Self) -> Self {
        Self { lo: &self.lo + &other.lo, hi: &self.hi + &other.hi }
    }

    #[must_use]
    pub fn sub(&self, other: &Self) -> Self {
        Self { lo: &self.lo - &other.hi, hi: &self.hi - &other.lo }
    }

    #[must_use]
    pub fn mul(&self, other: &Self) -> Self {
        self.corners(other, |a, b| a * b)
    }

    #[must_use]
    pub fn neg(&self) -> Self {
        Self { lo: -&self.hi, hi: -&self.lo }
    }

    /// The divisor's range without zero: its negative part and its positive part.
    fn nonzero_parts(&self) -> impl Iterator<Item = Self> {
        let below = Self::closed(self.lo.clone(), (&self.hi).min(&Integer::from(-1)).clone());
        let above = Self::closed((&self.lo).max(&Integer::from(1)).clone(), self.hi.clone());
        below.into_iter().chain(above)
    }

    fn divisor(&self) -> Divisor {
        if self.lo <= Integer::zero() && Integer::zero() <= self.hi {
            Divisor::MayBeZero
        } else {
            Divisor::NonZero
        }
    }

    /// Division rounding toward zero: over each sign of the divisor, the quotient's extremes are
    /// at the corners.
    #[must_use]
    #[expect(clippy::missing_panics_doc, reason = "the divisor's parts exclude zero")]
    pub fn div(&self, divisor: &Self) -> Divided {
        let parts =
            divisor.nonzero_parts().map(|d| self.corners(&d, |a, b| a.checked_div(b).expect("no zero")));
        Divided { range: parts.reduce(|a, b| a.hull(&b)), divisor: divisor.divisor() }
    }

    /// The remainder of [`Self::div`]: its sign follows the dividend and its magnitude is below
    /// the divisor's, so it lies in `[max(min(lo, 0), 1 - m), min(max(hi, 0), m - 1)]` where `m`
    /// is the largest divisor magnitude.
    #[must_use]
    pub fn rem(&self, divisor: &Self) -> Divided {
        let largest = divisor.nonzero_parts().map(|d| d.lo.abs().max(d.hi.abs())).max();
        let range = largest.map(|m| {
            let one = Integer::from(1);
            let lo = self.lo.clone().min(Integer::zero()).max(&one - &m);
            let hi = self.hi.clone().max(Integer::zero()).min(&m - &one);
            Self { lo, hi }
        });
        Divided { range, divisor: divisor.divisor() }
    }

    /// Whether a value can be negative, so that it is stored in two's complement.
    #[must_use]
    pub fn is_signed(&self) -> bool {
        self.lo.is_negative()
    }

    /// The fewest bits that hold every value directly, in two's complement when signed: the width
    /// of a `layout.bits` field (rule 7).
    #[must_use]
    pub fn bits(&self) -> u64 {
        if self.is_signed() {
            let below = (&-&self.lo - &Integer::from(1)).magnitude_bits();
            let above = if self.hi.is_negative() { 0 } else { self.hi.magnitude_bits() };
            below.max(above) + 1
        } else {
            self.hi.magnitude_bits()
        }
    }

    /// Where the range is stored, or `None` past 128 bits, the width cap (rule 5).
    #[must_use]
    pub fn storage(&self) -> Option<Storage> {
        let bits = self.bits();
        Storage::ALL.into_iter().find(|s| bits <= s.bits())
    }
}
