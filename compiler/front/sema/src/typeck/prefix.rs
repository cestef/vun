//! Prefix forms that keep or wrap their operand's type: `&e` borrows through the table
//! `borrows` (appendix B), `unsafe e` has the type of `e`, and so does `#[attrs] e`, whose
//! attributes are data. A borrow of a type, as in `Pair(&u8)`, is a type.

use overt_grammar::BORROWS;
use overt_hir::tree::ExprId;
use overt_intern::Symbol;

use super::{Known, Typer};
use crate::Primitive;
use crate::primitives::lang::Lang;
use crate::ty::{TyKind, Value};

impl Typer<'_, '_, '_> {
    /// `&mode operand`.
    pub(super) fn borrow(&mut self, expr: ExprId, mode: Option<Symbol>, operand: ExprId) -> Option<Known> {
        let known = self.typed_as(operand, self.expected(expr).and_then(|t| t.pointee()))?;
        let ty = TyKind::primitive(Primitive::Lang(Lang::r#type));
        if known.ty == ty {
            return Some(known);
        }
        let word = mode.map_or("", Symbol::as_str);
        let (_, item) = BORROWS.iter().find(|(m, _)| *m == word)?;
        let borrow =
            self.env.apply(self.env.item(item).ok()?, vec![Value::Type(known.ty)]).ok()?.as_type()?;
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
