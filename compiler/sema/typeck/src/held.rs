//! What a binding holds by value: never a slice, whose items are reached through a borrow (spec
//! 02, section 1), so a slice bound whole is reported here rather than found to have no layout.

use vun_diag::{Diagnostic, Fix, Msg, Safety, errors};
use vun_grammar::TokenKind;
use vun_hir::tree::{Decl, ExprId};
use vun_origin::OriginTable;
use vun_sema_ty::Ty;
use vun_span::Span;

use super::{Known, Stage, Typer};

impl Typer<'_, '_, '_> {
    /// Reports `d` binding the run-time value `found`, of type `ty`, when it is a slice.
    pub(super) fn held(&mut self, d: &Decl, ty: Option<Ty>, found: Option<Known>) {
        let (Some(ty), Some(value)) = (ty, d.value) else { return };
        if ty.slice_items().is_none() || found.is_some_and(|k| k.stage == Stage::Compile) {
            return;
        }
        let msg = Msg::new().text("a binding cannot hold the slice ").ty(ty.to_string());
        let note =
            "a slice's length is known only when the program runs, so it has no size a binding can hold";
        let fix = self.borrowing(value);
        let d = Diagnostic::error(errors::TYPES_SLICE_VALUE, msg).at(self.hir().exprs[value].origin, None);
        self.diag.error(d.note(note).fix(fix));
    }

    /// The fix borrowing `value` where it is written.
    fn borrowing(&self, value: ExprId) -> Fix {
        let span = OriginTable::global().source_span(self.hir().exprs[value].origin);
        let at = Span::new(span.file, span.range.lo, span.range.lo);
        let amp = TokenKind::Amp.text().unwrap_or_default();
        Fix::replace(Msg::new().text("borrow it: ").code(amp), Safety::Safe, at, amp).preferred()
    }
}
