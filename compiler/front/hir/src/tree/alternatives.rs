//! The binders of an or-pattern's alternatives, paired by name: every alternative binds the same
//! names with the same types (spec 01, section 6), and the first alternative's binders stand for
//! all, as names after the pattern resolve to them.

use super::{BinderId, Hir, PatId};

/// A binder of an alternative after the first, against the first alternative's binders.
#[derive(Clone, Copy, Debug, PartialEq, Eq, vun_codec::Encode, vun_codec::Decode)]
pub enum Joined {
    /// `other` binds the name `first` binds.
    Paired { first: BinderId, other: BinderId },
    /// `other` binds a name the first alternative does not.
    Extra { other: BinderId },
    /// The alternative `alternative` does not bind the name `first` binds.
    Missing { first: BinderId, alternative: PatId },
}

impl Hir {
    /// How the binders of every alternative after the first pair with the first's.
    #[must_use]
    pub fn joined(&self, alternatives: &[PatId]) -> Vec<Joined> {
        let Some((&head, rest)) = alternatives.split_first() else { return Vec::new() };
        let mut firsts = Vec::new();
        self.binders(head, &mut firsts);
        let mut joined = Vec::new();
        for &alternative in rest {
            let mut others = Vec::new();
            self.binders(alternative, &mut others);
            let named = |b: BinderId, among: &[BinderId]| {
                among.iter().copied().find(|&a| self.binders[a].name == self.binders[b].name)
            };
            joined.extend(others.iter().map(|&other| match named(other, &firsts) {
                Some(first) => Joined::Paired { first, other },
                None => Joined::Extra { other },
            }));
            let missing = firsts.iter().filter(|&&first| named(first, &others).is_none());
            joined.extend(missing.map(|&first| Joined::Missing { first, alternative }));
        }
        joined
    }
}
