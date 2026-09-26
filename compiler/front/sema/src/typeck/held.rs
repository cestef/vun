//! What a binding holds by value: never a slice, whose items are reached through a borrow (spec
//! 02, section 1), so a slice bound whole is reported here rather than found to have no layout.

use vun_diag::{Diagnostic, Fix, Msg, Safety, errors};
use vun_hir::tree::{Decl, ExprId};
use vun_origin::OriginTable;
use vun_span::Span;

use super::{Known, Stage, Typer};
use crate::ty::Ty;

impl Typer<'_, '_, '_> {
    /// Reports `d` binding the run-time value `found`, of type `ty`, when it is a slice.
    pub(super) fn held(&mut self, d: &Decl, ty: Option<Ty>, found: Option<Known>) {
        let (Some(ty), Some(value)) = (ty, d.value) else { return };
        if ty.slice_items().is_none() || found.is_some_and(|k| k.stage == Stage::Compile) {
            return;
        }
        let msg = Msg::new().text("a slice is reached through a borrow, not held: ").ty(ty.to_string());
        let d = Diagnostic::error(errors::TYPES_SLICE_VALUE, msg).at(self.hir().exprs[value].origin, None);
        self.diag.error(d.fix(self.borrowing(value)));
    }

    /// The fix borrowing `value` where it is written.
    fn borrowing(&self, value: ExprId) -> Fix {
        let span = OriginTable::global().source_span(self.hir().exprs[value].origin);
        let at = Span::new(span.file, span.range.lo, span.range.lo);
        Fix::replace(Msg::new().text("borrow it"), Safety::Safe, at, "&")
    }
}
