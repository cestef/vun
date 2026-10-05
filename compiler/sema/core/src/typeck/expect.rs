//! What typing reports of a value: its type is not the one expected, or a run-time integer is past
//! the width cap.

use std::sync::Arc;

use vun_desugar::{Borrow, MODE_SEPARATOR};
use vun_desugar::{COMPOUND, Narrowing};
use vun_diag::{ChildKind, Diagnostic, Fix, Msg, Safety, errors};
use vun_grammar::TokenKind;
use vun_hir::resolve::Res;
use vun_hir::tree::{ExprId, ExprKind};
use vun_origin::OriginId;
use vun_origin::OriginTable;
use vun_sema_int::{Interval, Storage};
use vun_sema_ty::trap::Unevaluated;
use vun_sema_ty::{Ty, TyKind, Value};
use vun_span::Span;

use super::reported::Reads;
use super::{GaveUp, Known, Stage, TypedBody, Typer};
use crate::env::Env;

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
            let note = Msg::new().text("the value is ").ty(found.ty.to_string());
            // A type written as an alias is shown with what it stands for; otherwise it is said.
            let note = if written == expected.to_string() {
                note.text(", which is not within ").ty(written.clone())
            } else {
                note.text(", and ").code(&written).text(" is ").ty(expected.to_string())
            };
            // A run-time value can be narrowed; a constant is changed, or the type widened.
            let help = if found.stage == Stage::Run {
                // The compound assignments that narrow without trapping, from the table `compound`.
                let operators = [Narrowing::Wrap, Narrowing::Sat].map(|n| {
                    COMPOUND
                        .iter()
                        .find(|row| row.narrowing == n)
                        .and_then(|row| row.token.text())
                        .unwrap_or_default()
                });
                Msg::new()
                    .text("widen ")
                    .code(&written)
                    .text(" to hold the whole range, or narrow the value explicitly; counters can narrow as they update, with ")
                    .code_list(operators)
            } else {
                Msg::new()
                    .text("use a value within ")
                    .code(&written)
                    .text(", or widen it if it is meant to hold this one")
            };
            let d = Diagnostic::error(errors::INT_DOES_NOT_FIT, msg)
                .child(ChildKind::Note, note)
                .child(ChildKind::Help, help);
            match self.compared_place(at) {
                Some(place) => d.child(ChildKind::Help, Self::unnarrowed(&place)),
                None => d,
            }
        } else {
            let msg = Msg::new().text("expected ").code(&written).text(", found ").ty(found.ty.to_string());
            let d = Diagnostic::error(errors::TYPES_MISMATCH, msg);
            match self.conversion(at, found.ty, expected) {
                Some(fix) => d.fix(fix),
                None if because.is_some() => d.help("change the value, or the type it is expected to have"),
                None => d,
            }
        };
        let mut d = d.at(at, Some(Msg::new().text("this is ").ty(found.ty.to_string())));
        if let Some(because) = because {
            let here = Msg::new().text("expected because of this type");
            d = d.label(self.hir().exprs[because].origin, Some(here));
        }
        if let Some(origin) = self.declaration_of(at) {
            d = d.label(origin, Some(Msg::new().text("declared here")));
        }
        self.diag.error(d);
    }

    /// Where the binding the value written at `at` names is declared, when it is a name.
    fn declaration_of(&self, at: OriginId) -> Option<OriginId> {
        let written = self.written_at(at)?;
        match self.env().parsed.resolution.names[written] {
            Some(Res::Binder(binder)) => Some(self.hir().binders[binder].origin),
            _ => None,
        }
    }

    /// The fix making the value written at `at`, of type `found`, one of `expected`, when a
    /// borrow does: borrowing it, taking away a borrow written around what already is the borrow
    /// expected, or reading through the borrow it is.
    fn conversion(&self, at: OriginId, found: Ty, expected: Ty) -> Option<Fix> {
        let span = OriginTable::global().source_span(at);
        if let Some((mode, _, borrowed)) = expected.borrowing()
            && borrowed.accepts(&found)
        {
            let amp = TokenKind::Amp.text().unwrap_or_default();
            let words = mode.to_string();
            let prefix =
                if words.is_empty() { amp.to_owned() } else { [amp, &words, MODE_SEPARATOR].concat() };
            // Borrowing shared is what the type asks; a mode that changes the place may not be.
            let safety = if mode == Borrow::plain() { Safety::Safe } else { Safety::Unsafe };
            let msg = Msg::new().text("borrow it: ").code(prefix.clone());
            return Some(Fix::insert(msg, safety, span, prefix).preferred());
        }
        let borrowed = found.borrowed()?;
        if let Some(fix) = self.unborrowing(at, expected) {
            return Some(fix);
        }
        expected.accepts(&borrowed).then(|| {
            let deref = TokenKind::DotStar.text().unwrap_or_default();
            let end = Span::new(span.file, span.range.hi, span.range.hi);
            let msg = Msg::new().text("read the value it borrows: ").code(deref);
            Fix::insert(msg, Safety::Unsafe, end, deref).preferred()
        })
    }

    /// What to do about the mutable `place`, compared above without being narrowed (spec 04,
    /// rule 4): a binding of it is narrowed.
    fn unnarrowed(place: &str) -> Msg {
        let name = place.rsplit('.').next().unwrap_or(place).trim_start_matches('_');
        Msg::new()
            .code(place)
            .text(" is a mutable place, which a comparison does not narrow; bind it first, ")
            .code(format!("{name} := {place};"))
            .text(", and use the binding")
    }

    /// The fix taking away the borrow written at `at`, when what it borrows is already accepted
    /// where `expected` is: `&mut into[n..]` where `into[n..]` is the `&mut [u8]` expected.
    fn unborrowing(&self, at: OriginId, expected: Ty) -> Option<Fix> {
        let hir = self.hir();
        let written = self.written_at(at)?;
        let ExprKind::Borrow { operand, .. } = hir.exprs[written].kind else { return None };
        let inner = self.found.types.get(&operand)?;
        if !expected.accepts(&inner.ty) {
            return None;
        }
        let table = OriginTable::global();
        let (span, operand_span) = (table.source_span(at), table.source_span(hir.exprs[operand].origin));
        let prefix = Span::new(span.file, span.range.lo, operand_span.range.lo);
        let msg =
            Msg::new().text("it is already ").ty(inner.ty.to_string()).text(", so take the borrow away");
        Some(Fix::replace(msg, Safety::Safe, prefix, "").preferred())
    }

    /// The type `expr` denotes, reporting a trap that fires while evaluating it.
    pub(super) fn ty(&mut self, expr: ExprId) -> Option<Ty> {
        self.annotation(expr).ok()
    }

    /// The type the annotation `expr` denotes, or why it has none: its value is not known here,
    /// or is not a type, which the check of the file reports.
    pub(super) fn annotation(&mut self, expr: ExprId) -> Result<Ty, GaveUp> {
        let evaluated = self.known_value(expr, None);
        if let Err(why) = &evaluated {
            self.needed_when_compiling(expr, why);
        }
        self.reported(expr, evaluated).map_err(GaveUp::Unevaluated)?.as_type().ok_or(GaveUp::Reported)
    }

    /// Reports `expr`, which is needed when compiling (an annotation, the length of an array), if
    /// it stopped, for `why`, because it reads a local its body computes when it runs.
    pub(super) fn needed_when_compiling(&mut self, expr: ExprId, why: &Unevaluated) {
        if !matches!(why, Unevaluated::Runtime) {
            return;
        }
        let hir = self.hir();
        if let Some((read, local)) = Reads::local(&self.evaluating(), expr) {
            self.diag.error(TypedBody::runtime(hir, local, hir.exprs[read].origin, false));
        }
    }

    /// The value at compile time of `expr`, an annotation or a `$` argument, as a value of `ty`
    /// where given; a trap that fires is the caller's to report, once, where it surfaces. A form that evaluates only once typed is typed
    /// then, at compile time: one typed by its context, such as `.square(5)` in
    /// `Int(0..=area(.square(5)))`, and a call leaving `$` arguments to inference, such as
    /// `TypeOf(x)`.
    pub(super) fn known_value(&mut self, expr: ExprId, ty: Option<Ty>) -> Result<Value, Unevaluated> {
        let evaluated = match self.evaluating().eval_as(expr, ty) {
            Err(Unevaluated::Unsupported | Unevaluated::Unbuilt(_))
                if !self.found.types.contains_key(&expr) =>
            {
                let outer = std::mem::replace(&mut self.stage, Stage::Compile);
                self.typed_as(expr, ty);
                self.stage = outer;
                self.evaluating().eval_as(expr, ty)
            }
            evaluated => evaluated,
        };
        if let Err(why) = &evaluated {
            self.unevaluated(expr, why);
        }
        evaluated
    }

    /// Traces an evaluation for `expr` that gave no value, where it is written and why, and
    /// counts one that read an unbound `$` parameter.
    pub(super) fn unevaluated(&mut self, expr: ExprId, why: &Unevaluated) {
        self.unbound += usize::from(matches!(why, Unevaluated::Unbound));
        vun_trace::tracing::trace!(at = %self.hir().exprs[expr].origin.located(), ?why, "unevaluated");
    }

    /// The environment every evaluation while typing runs in: with what typing found so far,
    /// shared, not copied. Forms typed by their context, methods called on values of no type of
    /// their own (arrays) and calls whose `$` arguments were inferred (`TypeOf(x)`) evaluate
    /// only with it.
    pub(super) fn evaluating(&self) -> Env<'a, 'db> {
        self.env().clone().typed_by(Some(Arc::clone(&self.found)))
    }

    /// What evaluating at compile time for `expr` gave, a trap that fired in it reported at
    /// `expr`, the root that needed the value.
    pub(super) fn reported<T>(
        &mut self,
        expr: ExprId,
        evaluated: Result<T, Unevaluated>,
    ) -> Result<T, Unevaluated> {
        match &evaluated {
            Err(Unevaluated::Trapped(trap)) => {
                self.diag.error(trap.report(self.hir().exprs[expr].origin));
            }
            Err(why) => self.unevaluated(expr, why),
            Ok(_) => {}
        }
        evaluated
    }

    /// What a `$` form at `expr` gave, typed as `known`: a trap that fired is reported, and
    /// where the form surely runs, a form compile-time evaluation does not run yet
    /// ([`Unevaluated::report`]).
    pub(super) fn ran<T>(
        &mut self,
        expr: ExprId,
        known: Option<Known>,
        evaluated: Result<T, Unevaluated>,
    ) -> Result<T, Unevaluated> {
        let surely = self.undecided == 0 && known.is_some();
        if let Err(stopped) = &evaluated
            && let Some(d) = stopped.report(self.hir().exprs[expr].origin, surely)
        {
            self.diag.error(d);
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
}
