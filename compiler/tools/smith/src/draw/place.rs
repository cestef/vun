//! Assignments to places: a local, what a reference lends, and the parts of either.

use super::Draw;
use crate::int::{BigInt, Range};
use crate::ir::{Assign, Expr, LocalId, Place, Projection, Stmt};
use crate::ty::Ty;

impl Draw<'_> {
    /// The locals a statement may write: mutable ones, and those holding a `&mut`, which lend
    /// what they refer to; not those a loop steps through.
    pub(super) fn writable(&self) -> Vec<(LocalId, bool)> {
        self.scope
            .visible
            .iter()
            .copied()
            .filter(|id| !self.scope.frozen.contains(id))
            .filter_map(|id| {
                let local = &self.scope.locals[id];
                match &local.ty {
                    Ty::Ref(true, _) => Some((id, true)),
                    Ty::Ref(false, _) => None,
                    _ => local.mutable.then_some((id, false)),
                }
            })
            .collect()
    }

    /// A place of a writable local, deep into its parts, and its type.
    pub(super) fn place(&mut self, depth: usize) -> Option<(Place, Ty)> {
        let roots = self.writable();
        if roots.is_empty() {
            return None;
        }
        let (local, deref) = *self.entropy.pick(&roots);
        let root = self.scope.locals[local].ty.clone();
        let mut ty = match (&root, deref) {
            (Ty::Ref(_, inner), true) => (**inner).clone(),
            _ => root.clone(),
        };
        let mut steps = Vec::new();
        while self.entropy.chance(2, 3) {
            let step = match &ty {
                Ty::Tuple(parts) => Projection::Field(self.entropy.below(parts.len())),
                Ty::Struct(_, parts) => Projection::Member(self.entropy.below(parts.len())),
                Ty::Array(len, _) => {
                    let index = self.int(depth);
                    let index = self.coerced(index, &Range::new(0, BigInt::from(*len) - BigInt::from(1)));
                    Projection::Index(Box::new(index))
                }
                _ => break,
            };
            steps.push(step);
            ty = Place::new(local, &root, deref, steps.clone()).ok()?.1;
        }
        Place::new(local, &root, deref, steps).ok()
    }

    /// An assignment to a place, of a value that cannot trap.
    pub(super) fn assign(&mut self) -> Option<Stmt> {
        let (place, ty) = self.place(self.config.depth)?;
        let Some(range) = ty.range().cloned() else {
            let value = self.expr(&ty, self.config.depth);
            return Some(Stmt::Assign { place, op: Assign::Set, value });
        };
        let op = *self.entropy.pick(Assign::ALL);
        let value = self.int(self.config.depth);
        let value = match op.arith() {
            None => self.coerced(value, &range),
            Some(arith) => {
                // The operation of `x += y` is `x = (x + y).wrap(T)`, whose sum must be one an integer may be.
                let held = Expr::sample(&ty);
                let value = if Expr::arith(arith, held.clone(), value.clone()).is_ok() {
                    value
                } else {
                    self.coerced(value, &Range::new(0, 1))
                };
                Expr::arith(arith, held, value.clone()).ok()?;
                value
            }
        };
        Some(Stmt::Assign { place, op, value })
    }
}
