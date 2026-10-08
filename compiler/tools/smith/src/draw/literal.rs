//! Integers drawn from a range, with a bias to its edges and to zero, where code goes wrong.

use super::Draw;
use crate::int::{BigInt, Range};

/// Where in a range a literal is taken: each way is a choice of its own.
#[derive(Clone, Copy, vun_variants::Variants)]
enum Place {
    Least,
    Greatest,
    Zero,
    NearLeast,
    NearGreatest,
    Any,
}

impl Place {
    const fn weight(self) -> usize {
        match self {
            Self::Least | Self::Greatest => 2,
            Self::Zero | Self::NearLeast | Self::NearGreatest => 1,
            Self::Any => 5,
        }
    }
}

impl Draw<'_> {
    /// An integer of `range`.
    pub(super) fn literal_in(&mut self, range: &Range) -> BigInt {
        let weights: Vec<usize> = Place::ALL.iter().map(|p| p.weight()).collect();
        let place = Place::ALL[self.entropy.weighted(&weights)];
        let one = BigInt::from(1);
        let n = match place {
            Place::Least => range.lo.clone(),
            Place::Greatest => range.hi.clone(),
            Place::Zero => BigInt::ZERO,
            Place::NearLeast => &range.lo + &one,
            Place::NearGreatest => &range.hi - &one,
            Place::Any => {
                let wide = (BigInt::from(self.entropy.bits()) << 64) | BigInt::from(self.entropy.bits());
                &range.lo + wide % range.count()
            }
        };
        if range.contains(&n) { n } else { range.lo.clone() }
    }
}
