//! Sets of token and node kinds, as bits.

use crate::{NodeKind, TokenKind};

const _: () = assert!(TokenKind::ALL.len() <= 128 && NodeKind::ALL.len() <= 128, "kinds fit a set's bits");

/// The bits set in `bits`, ascending, found without looking at those clear.
pub(super) fn bits(mut bits: u128) -> impl Iterator<Item = usize> {
    std::iter::from_fn(move || {
        let bit = bits.trailing_zeros() as usize;
        bits &= bits.checked_sub(1)?;
        Some(bit)
    })
}

/// A set of token kinds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TokenSet(u128);

/// A set of node kinds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KindSet(u128);

macro_rules! set {
    ($set:ident, $kind:ident) => {
        // A kind's bit is its value, and `ALL` lists the kinds by value, so a bit names its kind.
        const _: () = {
            let mut i = 0;
            while i < $kind::ALL.len() {
                assert!($kind::ALL[i] as usize == i, "kinds are listed by value");
                i += 1;
            }
        };

        impl $set {
            pub const EMPTY: Self = Self(0);

            #[must_use]
            pub fn of(kinds: &[$kind]) -> Self {
                kinds.iter().fold(Self::EMPTY, |set, &k| set.with(k))
            }

            #[must_use]
            pub const fn with(self, kind: $kind) -> Self {
                Self(self.0 | 1 << kind as usize)
            }

            #[must_use]
            pub const fn contains(self, kind: $kind) -> bool {
                self.0 & (1 << kind as usize) != 0
            }

            #[must_use]
            pub const fn union(self, other: Self) -> Self {
                Self(self.0 | other.0)
            }

            #[must_use]
            pub const fn intersection(self, other: Self) -> Self {
                Self(self.0 & other.0)
            }

            #[must_use]
            pub const fn without(self, other: Self) -> Self {
                Self(self.0 & !other.0)
            }

            #[must_use]
            pub const fn is_empty(self) -> bool {
                self.0 == 0
            }

            /// The kinds, in the order declared, which is their bits' order.
            pub fn iter(self) -> impl Iterator<Item = $kind> {
                bits(self.0).map(|bit| $kind::ALL[bit])
            }
        }

        impl FromIterator<$kind> for $set {
            fn from_iter<I: IntoIterator<Item = $kind>>(kinds: I) -> Self {
                kinds.into_iter().fold(Self::EMPTY, Self::with)
            }
        }
    };
}

set!(TokenSet, TokenKind);
set!(KindSet, NodeKind);
