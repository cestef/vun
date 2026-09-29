//! How tightly each form holds its operands, which kind wins where several fit, and which kinds
//! stand as operands.

use vun_tree_sitter::Assoc;

use super::{PARTS, Rules};
use crate::grammar::production::{Resolved, Term};

impl<'s> Rules<'_, 's> {
    /// How many kinds are preferred to `kind`, one before another: the longest chain of kinds
    /// listed before it in sets of `preferred`.
    pub(super) fn rank(&self, kind: &str) -> i32 {
        let before =
            self.preferred.iter().filter_map(|set| Some(&set[..set.iter().position(|k| *k == kind)?]));
        before.flatten().map(|k| self.rank(k) + 1).max().unwrap_or(0)
    }

    /// A level above every operator's.
    pub(super) fn above_operators(&self) -> i32 {
        self.levels.values().max().copied().unwrap_or_default() + 1
    }

    /// Right for a form ending with an operand, which may go on, left for another.
    pub(super) fn assoc(&self, terms: &[Term<'s, Resolved<'s>>]) -> Assoc {
        let open = match terms.last() {
            Some(Term::Field(field, None)) => self.holds_operands(field),
            Some(Term::Optional(_)) => true,
            _ => false,
        };
        if open { Assoc::Right } else { Assoc::Left }
    }

    /// Whether the field `field` holds any operand: it is named after no kind and no table says
    /// it holds only some.
    pub(super) fn holds_operands(&self, field: &str) -> bool {
        let named = self.nodes.variants.iter().any(|v| v.name == field);
        !named && self.contents.get(field).is_none_or(|(_, operands)| *operands)
    }

    /// Whether `kind` stands as an operand: it is no part, and no root.
    pub(super) fn is_operand(&self, kind: &str) -> bool {
        let root = self.produced.iter().any(|(k, terms)| *k == kind && self.is_root(terms));
        !root && self.kinds(PARTS).is_ok_and(|parts| !parts.contains(&kind))
    }
}
