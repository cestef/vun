//! Interval endpoints: an integer, or no bound on one side. `Int(1..)` is unbounded above; such
//! ranges have no storage and exist at compile time only (spec 04, rule 5).

use std::cmp::Ordering;

use super::Integer;

/// An integer or an infinity, ordered as on the number line.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Extended {
    NegInf,
    Finite(Integer),
    PosInf,
}

impl Extended {
    #[must_use]
    pub fn finite(&self) -> Option<&Integer> {
        match self {
            Self::Finite(n) => Some(n),
            Self::NegInf | Self::PosInf => None,
        }
    }

    fn sign(&self) -> Ordering {
        match self {
            Self::NegInf => Ordering::Less,
            Self::Finite(n) => n.cmp(&Integer::zero()),
            Self::PosInf => Ordering::Greater,
        }
    }

    /// The limit of a product of values of signs `a` and `b`, one of them infinite: zero when
    /// either is zero, otherwise the infinity of the product's sign.
    fn product_limit(a: Ordering, b: Ordering) -> Self {
        match (a, b) {
            (Ordering::Equal, _) | (_, Ordering::Equal) => Self::Finite(Integer::zero()),
            _ if a == b => Self::PosInf,
            _ => Self::NegInf,
        }
    }

    /// The sum; the two infinities never meet, since a sum adds two lower or two upper bounds.
    #[must_use]
    pub fn add(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::Finite(a), Self::Finite(b)) => Self::Finite(a + b),
            (Self::NegInf, _) | (_, Self::NegInf) => Self::NegInf,
            (Self::PosInf, _) | (_, Self::PosInf) => Self::PosInf,
        }
    }

    #[must_use]
    pub fn neg(&self) -> Self {
        match self {
            Self::NegInf => Self::PosInf,
            Self::Finite(n) => Self::Finite(-n),
            Self::PosInf => Self::NegInf,
        }
    }

    /// The product, with zero times an infinity being zero, as interval endpoints need.
    #[must_use]
    pub fn mul(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::Finite(a), Self::Finite(b)) => Self::Finite(a * b),
            _ => Self::product_limit(self.sign(), other.sign()),
        }
    }

    /// The quotient rounded toward zero, for a divisor that is not zero: a finite value over an
    /// infinity is zero, an infinity over anything keeps the sign of the quotient.
    ///
    /// # Panics
    /// Panics if the divisor is zero.
    #[must_use]
    pub fn div(&self, divisor: &Self) -> Self {
        match (self, divisor) {
            (Self::Finite(a), Self::Finite(b)) => Self::Finite(a.checked_div(b).expect("a nonzero divisor")),
            (Self::Finite(_), _) => Self::Finite(Integer::zero()),
            _ => Self::product_limit(self.sign(), divisor.sign()),
        }
    }
}
