//! `match` of an enum: an arm for each variant, or for some and then `_`.

use super::Draw;
use crate::ir::{Arm, Expr, LocalId, Pattern};
use crate::ty::Ty;

impl Draw<'_> {
    /// A `match` of an enum whose arms are of type `want`, or integers of any range where none is
    /// asked for; `None` where the program has no enum, or the arms' types will not join.
    pub(super) fn matched(&mut self, want: Option<&Ty>, depth: usize) -> Option<Expr> {
        let scrutinee = self.enum_value(depth)?;
        let Ty::Enum(_, variants) = scrutinee.ty.clone() else { return None };
        // The variants with an arm of their own: all of them, or those before a `_`.
        let named = if variants.len() > 1 && self.entropy.chance(1, 3) {
            self.entropy.between(0, variants.len() - 1)
        } else {
            variants.len()
        };
        let mut arms = Vec::new();
        for (variant, payload) in variants.iter().enumerate().take(named) {
            let seen = self.scope.visible.len();
            let binds: Vec<Option<LocalId>> = payload
                .iter()
                .map(|ty| self.entropy.chance(3, 4).then(|| self.scope.bind(ty.clone(), false)))
                .collect();
            let body = self.arm(want, depth);
            self.scope.visible.truncate(seen);
            arms.push(Arm { pattern: Pattern::Variant { variant, binds }, body });
        }
        if named < variants.len() {
            let body = self.arm(want, depth);
            arms.push(Arm { pattern: Pattern::Any, body });
        }
        Expr::matching(scrutinee, arms).ok()
    }

    fn arm(&mut self, want: Option<&Ty>, depth: usize) -> Expr {
        match want {
            Some(ty) => self.expr(ty, depth),
            None => self.int(depth),
        }
    }

    /// A value of an enum of the program: a read of a local that holds one, or one made.
    fn enum_value(&mut self, depth: usize) -> Option<Expr> {
        if self.entropy.chance(1, 2)
            && let Some(local) = self.local_of(&|ty| matches!(ty, Ty::Enum(..)))
        {
            return Some(local);
        }
        let enums: Vec<Ty> =
            self.program.types.iter().map(|(_, t)| t.clone()).filter(|t| matches!(t, Ty::Enum(..))).collect();
        (!enums.is_empty()).then(|| {
            let ty = self.entropy.pick(&enums).clone();
            self.expr(&ty, depth)
        })
    }
}
