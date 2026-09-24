//! What typing reports: a value whose type is not the one expected, a run-time integer past the
//! width cap, and a member its type does not have.

use vun_diag::{ChildKind, Diagnostic, Msg, errors};
use vun_grammar::{COMPOUND, Narrowing, OPERATORS, TokenKind};
use vun_hir::tree::ExprId;
use vun_intern::Symbol;
use vun_origin::OriginId;

use std::sync::Arc;

use super::{Known, Stage, TypedBody, Typer};
use crate::env::{Env, Unevaluated};
use crate::int::{Interval, Storage};
use crate::ty::{Ty, TyKind};
use crate::visible::Visible;

impl<'a, 'db> Typer<'_, 'a, 'db> {
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
            // A run-time value can be narrowed; a constant only fits a wider type.
            let help = if found.stage == Stage::Run {
                // The compound assignments that narrow without trapping, from the table `compound`.
                let operators = [Narrowing::Wrap, Narrowing::Sat].map(|n| {
                    COMPOUND.iter().find(|(.., m)| *m == n).and_then(|(t, ..)| t.text()).unwrap_or_default()
                });
                Msg::new()
                    .text("widen ")
                    .code(&written)
                    .text(" to hold the whole range, or narrow the value explicitly; counters can narrow as they update, with ")
                    .code_list(operators)
            } else {
                Msg::new().text("widen ").code(&written).text(" to hold the value")
            };
            Diagnostic::error(errors::INT_DOES_NOT_FIT, msg)
                .child(ChildKind::Note, note)
                .child(ChildKind::Help, help)
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
        let evaluated = match self.env().ty(expr) {
            // Forms typed by their context, such as `.square(5)` in `Int(0..=area(.square(5)))`,
            // evaluate once the annotation is typed.
            Err(Unevaluated::Unsupported) => {
                let outer = std::mem::replace(&mut self.stage, Stage::Compile);
                self.typed(expr);
                self.stage = outer;
                self.evaluating().ty(expr)
            }
            evaluated => evaluated,
        };
        self.reported(expr, evaluated).ok()
    }

    /// The environment evaluating what is typed so far, with the types found for it: forms
    /// typed by their context, and methods called on values of no type of their own, such as
    /// arrays, dispatch by them.
    pub(super) fn evaluating(&self) -> Env<'a, 'db> {
        let types =
            TypedBody { types: self.types.iter().map(|(&e, &k)| (e, k)).collect(), ..TypedBody::default() };
        self.env().clone().typed_by(Some(Arc::new(types)))
    }

    /// What evaluating at compile time for `expr` gave, a trap that fired in it reported at
    /// `expr`, the root that needed the value.
    pub(super) fn reported<T>(
        &mut self,
        expr: ExprId,
        evaluated: Result<T, Unevaluated>,
    ) -> Result<T, Unevaluated> {
        if let Err(Unevaluated::Trapped(trap)) = &evaluated {
            self.diag.error(trap.report(self.hir().exprs[expr].origin));
        }
        evaluated
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

    /// Reports the operator `op` at `expr` as having no impl for `ty`, the type of its left side.
    pub(super) fn no_impl(&mut self, expr: ExprId, op: TokenKind, ty: Ty) {
        let Some((_, interface, _)) = OPERATORS.iter().find(|(t, ..)| *t == op) else { return };
        let msg = Msg::new().text("no impl of ").code(*interface).text(" for ").ty(ty.to_string());
        let note = Msg::new()
            .text("bit operations exist on ")
            .code("uN")
            .text(" types only: ")
            .code("Int(0..=2^N - 1)");
        let help =
            Msg::new().text("use an unsigned type, or bring the value into one with ").code(".wrap(uN)");
        let d = Diagnostic::error(errors::TYPES_NO_IMPL, msg)
            .at(self.hir().exprs[expr].origin, None)
            .child(ChildKind::Note, note)
            .child(ChildKind::Help, help);
        self.diag.error(d);
    }

    /// Reports `name`, used at `at`, as no member of `ty`.
    pub(super) fn no_member(&mut self, at: OriginId, ty: Ty, name: Symbol) {
        let msg = Msg::new().text("no member ").code(name.as_str()).text(" in ").ty(ty.to_string());
        self.diag.error(Diagnostic::error(errors::TYPES_NO_MEMBER, msg).at(at, None));
    }
}
