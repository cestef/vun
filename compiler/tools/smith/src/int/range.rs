//! A closed range of integers, and the types it names.

use num_bigint::BigInt;

/// The widest integer a run-time intermediate may need, in bits (spec 04, rule 5).
const WIDEST: u32 = 128;

/// The integers from `lo` to `hi`, both included.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Range {
    pub lo: BigInt,
    pub hi: BigInt,
}

/// An integer type of the `lang.int` table: signed or not, of so many bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Width {
    pub signed: bool,
    pub bits: u32,
}

impl Width {
    /// The widths `lang.int` names.
    pub const NAMED: [u32; 5] = [8, 16, 32, 64, 128];

    /// The range of this type.
    #[must_use]
    pub fn range(self) -> Range {
        if self.signed { Range::signed(self.bits) } else { Range::unsigned(self.bits) }
    }
}

impl Range {
    ///
    /// # Panics
    /// If `lo` is above `hi`.
    #[must_use]
    pub fn new(lo: impl Into<BigInt>, hi: impl Into<BigInt>) -> Self {
        let (lo, hi) = (lo.into(), hi.into());
        assert!(lo <= hi, "a range holds an integer");
        Self { lo, hi }
    }

    /// The type of the literal `n`.
    #[must_use]
    pub fn exactly(n: impl Into<BigInt>) -> Self {
        let n = n.into();
        Self { lo: n.clone(), hi: n }
    }

    /// `Int(0..2^bits)`
    #[must_use]
    pub fn unsigned(bits: u32) -> Self {
        Self::new(0, (BigInt::from(1) << bits) - BigInt::from(1))
    }

    /// `Int(-2^(bits-1)..2^(bits-1))`
    #[must_use]
    pub fn signed(bits: u32) -> Self {
        let half = BigInt::from(1) << (bits - 1);
        Self::new(-half.clone(), half - BigInt::from(1))
    }

    #[must_use]
    pub fn contains(&self, n: &BigInt) -> bool {
        &self.lo <= n && n <= &self.hi
    }

    /// Whether every integer of this range is in `other`: an `Int(self)` where an `Int(other)` is
    /// expected needs no conversion.
    #[must_use]
    pub fn within(&self, other: &Self) -> bool {
        other.lo <= self.lo && self.hi <= other.hi
    }

    /// The least range holding both: the type of an `if` of two branches.
    #[must_use]
    pub fn hull(&self, other: &Self) -> Self {
        Self { lo: self.lo.clone().min(other.lo.clone()), hi: self.hi.clone().max(other.hi.clone()) }
    }

    /// How many integers it holds.
    #[must_use]
    pub fn count(&self) -> BigInt {
        &self.hi - &self.lo + BigInt::from(1)
    }

    /// The integer type this range is, if `lang.int` names one.
    #[must_use]
    pub fn width(&self) -> Option<Width> {
        [false, true]
            .into_iter()
            .flat_map(|signed| Width::NAMED.map(|bits| Width { signed, bits }))
            .find(|w| w.range() == *self)
    }

    /// The bits `N` of the `uN` this range is, if it is one: the types bit operations are on.
    #[must_use]
    pub fn word(&self) -> Option<u32> {
        let bits = u32::try_from(self.hi.bits()).ok()?;
        (self.lo == BigInt::ZERO && *self == Self::unsigned(bits) && bits > 0).then_some(bits)
    }

    /// The fewest bits an integer of this range needs, with a sign bit if any is negative.
    #[must_use]
    pub fn needs(&self) -> u32 {
        let magnitude = |n: &BigInt| {
            let n = if n.sign() == num_bigint::Sign::Minus { -n - BigInt::from(1) } else { n.clone() };
            u32::try_from(n.bits()).unwrap_or(u32::MAX)
        };
        let bits = magnitude(&self.lo).max(magnitude(&self.hi));
        if self.lo.sign() == num_bigint::Sign::Minus { bits + 1 } else { bits.max(1) }
    }

    /// Whether a run-time value of this range is within the widest the compiler admits.
    #[must_use]
    pub fn fits_a_word(&self) -> bool {
        self.needs() <= WIDEST
    }

    /// The bits of the smallest of 8, 16, 32, 64 or 128 that stores this range (spec 04, rule 7).
    #[must_use]
    pub fn storage(&self) -> u32 {
        Width::NAMED.into_iter().find(|&bits| self.needs() <= bits).unwrap_or(WIDEST)
    }
}

impl Range {
    /// `n` reduced modulo the size of the range into it (spec 04, rule 3).
    #[must_use]
    pub fn wrap(&self, n: &BigInt) -> BigInt {
        let count = self.count();
        let offset = (n - &self.lo) % &count;
        let offset = if offset.sign() == num_bigint::Sign::Minus { offset + &count } else { offset };
        &self.lo + offset
    }

    /// `n` clamped to the bounds.
    #[must_use]
    pub fn saturate(&self, n: &BigInt) -> BigInt {
        n.clone().max(self.lo.clone()).min(self.hi.clone())
    }
}
