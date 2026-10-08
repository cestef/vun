//! The range of each result (spec 04, rules 1, 6 and 8).

use num_bigint::{BigInt, Sign};

use super::Range;

impl Range {
    #[must_use]
    pub fn sum(&self, other: &Self) -> Self {
        Self { lo: &self.lo + &other.lo, hi: &self.hi + &other.hi }
    }

    #[must_use]
    pub fn difference(&self, other: &Self) -> Self {
        Self { lo: &self.lo - &other.hi, hi: &self.hi - &other.lo }
    }

    #[must_use]
    pub fn product(&self, other: &Self) -> Self {
        Self::of([&self.lo * &other.lo, &self.lo * &other.hi, &self.hi * &other.lo, &self.hi * &other.hi])
    }

    #[must_use]
    pub fn negation(&self) -> Self {
        Self { lo: -&self.hi, hi: -&self.lo }
    }

    /// The range of what `self / other` gives, rounding toward zero, leaving out a zero divisor.
    /// `None` when the divisor is only zero.
    #[must_use]
    pub fn quotient(&self, other: &Self) -> Option<Self> {
        let corners = other.nonzero().into_iter().flat_map(|d| {
            let (a, b) = (&self.lo, &self.hi);
            [a / &d.lo, a / &d.hi, b / &d.lo, b / &d.hi]
        });
        let all: Vec<BigInt> = corners.collect();
        (!all.is_empty()).then(|| Self::of(all))
    }

    /// The range of what `self % other` gives: of the dividend's sign, below the divisor's
    /// magnitude. `None` when the divisor is only zero.
    #[must_use]
    pub fn remainder(&self, other: &Self) -> Option<Self> {
        let most =
            other.nonzero().iter().map(|d| d.lo.magnitude().clone().max(d.hi.magnitude().clone())).max()?;
        let below = BigInt::from(most) - BigInt::from(1);
        let hi =
            if self.hi.sign() == Sign::Minus { BigInt::ZERO } else { self.hi.clone().min(below.clone()) };
        let lo = if self.lo.sign() == Sign::Minus { self.lo.clone().max(-below) } else { BigInt::ZERO };
        Some(Self { lo, hi })
    }

    /// The parts of this range that are not zero: a negative one, a positive one, or both.
    #[must_use]
    pub fn nonzero(&self) -> Vec<Self> {
        let (zero, one) = (BigInt::ZERO, BigInt::from(1));
        let mut parts = Vec::new();
        if self.lo < zero {
            parts.push(Self { lo: self.lo.clone(), hi: self.hi.clone().min(-&one) });
        }
        if self.hi > zero {
            parts.push(Self { lo: self.lo.clone().max(one), hi: self.hi.clone() });
        }
        parts
    }

    /// The range of a bit operation between two `uN` types: the wider, when the other lies in it.
    #[must_use]
    pub fn bitwise(&self, other: &Self) -> Option<Self> {
        let (a, b) = (self.word()?, other.word()?);
        Some(Self::unsigned(a.max(b)))
    }

    /// The range of `x << n` and `x >> n` for `x` a `uN` and `n` below `N`: the type of `x`.
    #[must_use]
    pub fn shifted(&self, amount: &Self) -> Option<Self> {
        let bits = self.word()?;
        amount.within(&Self::new(0, bits - 1)).then(|| self.clone())
    }

    /// The least range holding every one of `values`.
    fn of(values: impl IntoIterator<Item = BigInt>) -> Self {
        let values: Vec<BigInt> = values.into_iter().collect();
        Self {
            lo: values.iter().min().expect("a range has a corner").clone(),
            hi: values.iter().max().expect("a range has a corner").clone(),
        }
    }
}
