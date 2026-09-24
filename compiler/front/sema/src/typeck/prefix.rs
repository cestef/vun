//! Prefix forms that keep or wrap their operand's type: `&e` borrows through the table
//! `borrows` (appendix B), for the whole program when `e` lives that long; `unsafe e` has the
//! type of `e`, and so does `#[attrs] e`, whose attributes are data. A borrow of a type, as in
//! `Pair(&u8)`, is a type.

use vun_desugar::{Borrow, Extent};
use vun_hir::tree::{ExprId, ExprKind};

use super::{Known, Typer};
use crate::ty::{TyKind, Value};

impl Typer<'_, '_, '_> {
    /// `&mode operand`.
    pub(super) fn borrow(&mut self, expr: ExprId, mode: Borrow, operand: ExprId) -> Option<Known> {
        let pointee = self.expected(expr).and_then(|t| t.pointee());
        let mut known = self.typed_as(operand, pointee)?;
        // Of a type, such as `u8` or `[u8]`, `&` makes a borrow type (type syntax mirrors value
        // syntax, spec 01).
        if known.ty == TyKind::of_types() || self.env().denotes_type(operand) {
            return Some(Known { ty: TyKind::of_types(), ..known });
        }
        // A temporary, such as `7` in `&7`, holds the value as the type the borrow is expected
        // to reach when that type accepts it, as `x: u8 = 7` stores it; a place keeps its own.
        let place = matches!(
            self.hir().exprs[operand].kind,
            ExprKind::Name(_) | ExprKind::Member { .. } | ExprKind::Index { .. } | ExprKind::Postfix { .. }
        );
        if let Some(pointee) = pointee.filter(|p| !place && p.accepts(&known.ty)) {
            known = Known { ty: pointee, ..known };
            self.types.insert(operand, known);
        }
        // A borrow for the borrower's scope of what lives as long as the program is one for the
        // whole program: the row of the same transition and access whose extent is `static`.
        let mode = match mode.lasting() {
            Some(lasting) if mode.extent() == Extent::Tied && self.env().lasting(operand) => lasting,
            _ => mode,
        };
        let borrow = self
            .env()
            .apply(self.env().item(mode.item()).ok()?, vec![Value::Type(known.ty)])
            .ok()?
            .as_type()?;
        Some(Known { ty: borrow, ..known })
    }

    /// `#[attrs] inner`: the attributes are typed as values, and the whole has the type of
    /// `inner`.
    pub(super) fn attributed(&mut self, expr: ExprId, attrs: &[ExprId], inner: ExprId) -> Option<Known> {
        for &attr in attrs {
            self.typed(attr);
        }
        self.typed_as(inner, self.expected(expr))
    }
}
