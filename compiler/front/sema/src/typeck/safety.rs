//! `unsafe` (spec 07, section 4): something marked `unsafe` is used only where someone vouched
//! for it. An `unsafe` expression is vouched for where it is written, so the operations in it
//! may be unchecked; a function or function type marked `unsafe` is vouched for at each call.
//! Unchecked operations: calling a function whose type is marked, `.*` on a raw pointer, and any
//! access to a top-level `mut` binding; a `lang.mem.Cell` is reached only through `unsafe`
//! functions.

use vun_diag::{Diagnostic, Msg, errors};
use vun_hir::resolve::Res;
use vun_hir::tree::{ExprId, ExprKind};

use super::{Known, Typer};
use crate::Primitive;
use crate::primitives::mem::Mem;
use crate::ty::{Ty, TyKind};

impl Typer<'_, '_, '_> {
    /// `unsafe e`: a function literal or type is marked, anything else is vouched for.
    pub(super) fn vouching(&mut self, expr: ExprId, operand: ExprId) -> Option<Known> {
        // The literal's own type carries the marker (`Env::literal_signature`).
        if matches!(self.hir().exprs[operand].kind, ExprKind::Fn { .. }) {
            return self.typed(operand);
        }
        if matches!(self.hir().exprs[operand].kind, ExprKind::FnType { .. }) {
            self.typed(operand);
            return self.env().eval(expr).ok().and_then(|v| v.ty()).map(|ty| Known { ty, stage: self.stage });
        }
        self.vouched += 1;
        let known = self.typed_as(operand, self.expected(expr));
        self.vouched -= 1;
        known
    }

    /// Reports `what`, done at `expr`, unless an `unsafe` expression vouches for it.
    pub(super) fn unchecked(&mut self, expr: ExprId, what: Msg) {
        if self.vouched > 0 {
            return;
        }
        let msg = what.text(" is not checked by the compiler, and needs an ").code("unsafe").text(" block");
        let help = Msg::new()
            .text("wrap it in ")
            .code("unsafe { }")
            .text(", stating that what it relies on holds here");
        let d = Diagnostic::error(errors::SAFETY_REQUIRES_UNSAFE, msg)
            .at(self.hir().exprs[expr].origin, None)
            .child(vun_diag::ChildKind::Help, help);
        self.diag.error(d);
    }

    /// `.*` on a value of type `ty`: unchecked through a raw pointer.
    pub(super) fn raw_deref(&mut self, expr: ExprId, ty: Ty) {
        if matches!(&*ty, TyKind::Primitive(Primitive::Mem(Mem::Ptr), _)) {
            self.unchecked(expr, Msg::new().text("reading through a raw pointer"));
        }
    }

    /// A read or write of a name: unchecked for a top-level `mut` binding (spec 01, section 3).
    pub(super) fn mutable_static(&mut self, expr: ExprId) {
        let hir = self.hir();
        let Some(Res::Binder(b)) = self.env().parsed.resolution.names[expr] else { return };
        if hir.binders[b].mutable && hir.member_decl(b).is_some() {
            self.unchecked(
                expr,
                Msg::new()
                    .text("accessing the top-level ")
                    .code("mut")
                    .text(" binding ")
                    .code(hir.binders[b].name.as_str()),
            );
        }
    }
}
