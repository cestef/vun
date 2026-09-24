//! Prefix forms that keep or wrap their operand's type: `&e` borrows through the table
//! `borrows` (appendix B), for the whole program when `e` lives that long, `unsafe e` has the type of `e`, and so does `#[attrs] e`, whose
//! attributes are data. A borrow of a type, as in `Pair(&u8)`, is a type.

use vun_desugar::{BORROWS, Extent};
use vun_hir::tree::ExprId;
use vun_intern::Symbol;

use super::{Known, Typer};
use crate::ty::{TyKind, Value};

impl Typer<'_, '_, '_> {
    /// `&mode operand`.
    pub(super) fn borrow(&mut self, expr: ExprId, mode: Option<Symbol>, operand: ExprId) -> Option<Known> {
        let known = self.typed_as(operand, self.expected(expr).and_then(|t| t.pointee()))?;
        let ty = TyKind::of_types();
        if known.ty == ty {
            return Some(known);
        }
        let word = mode.map_or("", Symbol::as_str);
        let &(_, item, before, after, access, extent) = BORROWS.iter().find(|(m, ..)| *m == word)?;
        // A borrow for the borrower's scope of what lives as long as the program is one for the
        // whole program: the row of the same transition and access whose extent is `static`.
        let lasting = BORROWS
            .iter()
            .find(|&&(_, _, b, a, x, e)| (b, a, x, e) == (before, after, access, Extent::Static));
        let item = match lasting {
            Some(&(_, lasting, ..)) if extent == Extent::Tied && self.env().lasting(operand) => lasting,
            _ => item,
        };
        let borrow =
            self.env().apply(self.env().item(item).ok()?, vec![Value::Type(known.ty)]).ok()?.as_type()?;
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
