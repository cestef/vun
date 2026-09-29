//! How tightly each form holds its operands, which kind wins where several fit, and which kinds
//! stand as operands.

use vun_tree_sitter::Assoc;

use super::Rules;
use crate::grammar::model::{Field, Kind, Term};

impl Rules<'_> {
    /// How many kinds are preferred to `kind`, one before another: the longest chain of kinds
    /// listed before it in sets of `preferred`.
    pub(super) fn rank(&self, kind: Kind) -> i32 {
        let preferred = &self.model.preferred;
        let before = preferred.iter().filter_map(|set| Some(&set[..set.iter().position(|&k| k == kind)?]));
        before.flatten().map(|&k| self.rank(k) + 1).max().unwrap_or(0)
    }

    /// A level above every operator's.
    pub(super) fn above_operators(&self) -> i32 {
        self.levels.values().max().copied().unwrap_or_default() + 1
    }

    /// Right for a form ending with an operand, which may go on, left for another.
    pub(super) fn assoc(&self, terms: &[Term]) -> Assoc {
        let open = match terms.last() {
            Some(Term::Field(field, None)) => self.holds_operands(*field),
            Some(Term::Optional(_)) => true,
            _ => false,
        };
        if open { Assoc::Right } else { Assoc::Left }
    }

    /// Whether the field `field` holds any operand: it is named after no kind and no table says
    /// it holds only some.
    pub(super) fn holds_operands(&self, field: Field) -> bool {
        self.model.named_kind(field).is_none() && self.model.contents.get(&field).is_none_or(|c| c.operands)
    }

    /// Whether `kind` stands as an operand: it is no part, and no root.
    pub(super) fn is_operand(&self, kind: Kind) -> bool {
        let root = self.model.production(kind).is_some_and(|terms| self.is_root(terms));
        !root && !self.model.parts.contains(&kind) && kind != self.editors.tree_sitter.error
    }
}
