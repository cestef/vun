//! Sets of token and node kinds, as bits.

use crate::{NodeKind, TokenKind};

const _: () = assert!(TokenKind::ALL.len() <= 128 && NodeKind::ALL.len() <= 128, "kinds fit a set's bits");

/// A set of token kinds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TokenSet(u128);

/// A set of node kinds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KindSet(u128);

macro_rules! set {
    ($set:ident, $kind:ident) => {
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

            /// The kinds, in the order declared.
            pub fn iter(self) -> impl Iterator<Item = $kind> {
                $kind::ALL.iter().copied().filter(move |&k| self.contains(k))
            }
        }
    };
}

set!(TokenSet, TokenKind);
set!(KindSet, NodeKind);
