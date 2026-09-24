//! What typing reports: a value whose type is not the one expected, a run-time integer past the
//! width cap, and a member its type does not have.

use overt_diag::{ChildKind, Diagnostic, Msg, errors};
use overt_hir::tree::ExprId;
use overt_intern::Symbol;
use overt_origin::OriginId;

use super::{Known, Typer};
use crate::env::Unevaluated;
use crate::int::{Interval, Storage};
use crate::ty::{Ty, TyKind};
use crate::visible::Visible;

impl Typer<'_, '_, '_> {
    /// Checks that a value of `found`, written at `at`, is accepted where `expected` is, the type
    /// written at `because` when it is written.
    pub(super) fn expect(&mut self, at: OriginId, found: Known, expected: Ty, because: Option<ExprId>) {
        if expected.accepts(&found.ty) {
            return;
        }
        let written = because.map_or_else(|| expected.to_string(), |e| self.env().source(e));
        // Integers that do not fit are their own diagnostic (spec 04, rule 2).
        let d = if expected.interval().is_some() && found.ty.interval().is_some() {
            let msg = Msg::new().text("value does not fit the type ").code(&written);
            let note = Msg::new().text("the value is ").ty(found.ty.to_string()).text(", and ");
            let note = note.code(&written).text(" is ").ty(expected.to_string());
            Diagnostic::error(errors::INT_DOES_NOT_FIT, msg).child(ChildKind::Note, note)
        } else {
            let msg = Msg::new().text("expected ").code(&written).text(", found ").ty(found.ty.to_string());
            Diagnostic::error(errors::TYPES_MISMATCH, msg)
        };
        let mut d = d.at(at, None);
        if let Some(because) = because {
            let here = Msg::new().text("expected because of this type");
            d = d.label(self.hir().exprs[because].origin, Some(here));
        }
        self.diag.error(d);
    }

    /// The type `expr` denotes, reporting a trap that fires while evaluating it.
    pub(super) fn ty(&mut self, expr: ExprId) -> Option<Ty> {
        match self.env().ty(expr) {
            Ok(ty) => Some(ty),
            Err(Unevaluated::Trapped(trap)) => {
                self.diag.error(trap.report(self.hir().exprs[expr].origin));
                None
            }
            Err(_) => None,
        }
    }

    /// Reports the run-time intermediate `expr`, of range `range`, as needing more bits than the
    /// widest storage (spec 04, rule 5).
    pub(super) fn width_cap(&mut self, expr: ExprId, range: &Interval) {
        let bits = range.bits().map_or(0, i128::from);
        let cap = i128::from(Storage::CAP.bits());
        let msg =
            Msg::new().text("this intermediate needs ").num(bits).text(" bits, past the cap of ").num(cap);
        let note = Msg::new().text("its range is ").ty(TyKind::int(range).to_string());
        let help = Msg::new().text("narrow an operand before this operation");
        let d = Diagnostic::error(errors::INT_WIDTH_CAP, msg)
            .at(self.hir().exprs[expr].origin, None)
            .child(ChildKind::Note, note)
            .child(ChildKind::Help, help);
        self.diag.error(d);
    }

    /// Reports `name`, used at `expr`, when its visibility `visible` does not reach there.
    pub(super) fn check_visible(&mut self, expr: ExprId, name: Symbol, visible: &Visible) {
        if self.env().sees(visible, expr) != Some(false) {
            return;
        }
        self.diag.error(self.env().not_visible(self.hir().exprs[expr].origin, name, visible));
    }

    /// Reports `name`, used at `at`, as no member of `ty`.
    pub(super) fn no_member(&mut self, at: OriginId, ty: Ty, name: Symbol) {
        let msg = Msg::new().text("no member ").code(name.as_str()).text(" in ").ty(ty.to_string());
        self.diag.error(Diagnostic::error(errors::TYPES_NO_MEMBER, msg).at(at, None));
    }
}
