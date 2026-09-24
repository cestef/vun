//! What a comparison of integers tells of its operands' ranges (spec 04, rule 4).

use vun_desugar::Comparison;

use super::{Extended, Integer, Interval};

/// The meaning of a comparison on ranges.
pub trait Compared: Sized {
    /// The comparison holding exactly where this one fails.
    #[must_use]
    fn negated(self) -> Self;
    /// The comparison of the right operand to the left: `b > a` where `a < b`.
    #[must_use]
    fn flipped(self) -> Self;
    /// The values of `left` for which some value of `right` makes the comparison hold, `None`
    /// when there are none.
    fn narrowed(self, left: &Interval, right: &Interval) -> Option<Interval>;
}

impl Compared for Comparison {
    fn negated(self) -> Self {
        match self {
            Self::Less => Self::AtLeast,
            Self::AtMost => Self::Greater,
            Self::Greater => Self::AtMost,
            Self::AtLeast => Self::Less,
            Self::Equal => Self::Unequal,
            Self::Unequal => Self::Equal,
        }
    }

    fn flipped(self) -> Self {
        match self {
            Self::Less => Self::Greater,
            Self::AtMost => Self::AtLeast,
            Self::Greater => Self::Less,
            Self::AtLeast => Self::AtMost,
            Self::Equal | Self::Unequal => self,
        }
    }

    fn narrowed(self, left: &Interval, right: &Interval) -> Option<Interval> {
        let one = || Extended::Finite(Integer::from(1));
        let (lo, hi) = (left.lo().clone(), left.hi().clone());
        let bounds = match self {
            Self::Less => (lo, right.hi().add(&one().neg())),
            Self::AtMost => (lo, right.hi().clone()),
            Self::Greater => (right.lo().add(&one()), hi),
            Self::AtLeast => (right.lo().clone(), hi),
            Self::Equal => (right.lo().clone(), right.hi().clone()),
            // Only a single value at one of the left's bounds can be excluded from it.
            Self::Unequal if right.lo() == right.hi() && right.lo() == left.lo() => (lo.add(&one()), hi),
            Self::Unequal if right.lo() == right.hi() && right.hi() == left.hi() => {
                (lo, hi.add(&one().neg()))
            }
            Self::Unequal => (lo, hi),
        };
        Interval::new(bounds.0, bounds.1)?.meet(left)
    }
}
