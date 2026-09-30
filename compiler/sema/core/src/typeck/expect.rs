//! What typing reports: a value whose type is not the one expected, a run-time integer past the
//! width cap, and a member its type does not have.

use std::sync::Arc;

use vun_desugar::{Borrow, MODE_SEPARATOR};
use vun_desugar::{COMPOUND, Narrowing};
use vun_diag::{ChildKind, Diagnostic, Fix, Msg, Safety, errors};
use vun_grammar::TokenKind;
use vun_hir::tree::ExprId;
use vun_intern::Symbol;
use vun_items::Item;
use vun_origin::OriginId;
use vun_origin::OriginTable;
use vun_sema_int::{Interval, Storage};
use vun_sema_ty::trap::Unevaluated;
use vun_sema_ty::{Ty, TyKind, Value};
use vun_span::Span;
use vun_suggest::closest;

use super::{GaveUp, Known, Stage, Typer};
use crate::env::Env;
use crate::member::Members;
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
                Msg::new().text("widen ").code(&written).text(" to hold the value")
            };
            Diagnostic::error(errors::INT_DOES_NOT_FIT, msg)
                .child(ChildKind::Note, note)
                .child(ChildKind::Help, help)
        } else {
            let msg = Msg::new().text("expected ").code(&written).text(", found ").ty(found.ty.to_string());
            let d = Diagnostic::error(errors::TYPES_MISMATCH, msg);
            match Self::conversion(at, found.ty, expected) {
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
        self.diag.error(d);
    }

    /// The fix making the value written at `at`, of type `found`, one of `expected`, when a
    /// borrow does: borrowing it, or reading through the borrow it is.
    fn conversion(at: OriginId, found: Ty, expected: Ty) -> Option<Fix> {
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
        expected.accepts(&borrowed).then(|| {
            let deref = TokenKind::DotStar.text().unwrap_or_default();
            let end = Span::new(span.file, span.range.hi, span.range.hi);
            let msg = Msg::new().text("read the value it borrows: ").code(deref);
            Fix::insert(msg, Safety::Unsafe, end, deref).preferred()
        })
    }

    /// The type `expr` denotes, reporting a trap that fires while evaluating it.
    pub(super) fn ty(&mut self, expr: ExprId) -> Option<Ty> {
        self.annotation(expr).ok()
    }

    /// The type the annotation `expr` denotes, or why it has none: its value is not known here,
    /// or is not a type, which the check of the file reports.
    pub(super) fn annotation(&mut self, expr: ExprId) -> Result<Ty, GaveUp> {
        let evaluated = self.known_value(expr, None);
        self.reported(expr, evaluated).map_err(GaveUp::Unevaluated)?.as_type().ok_or(GaveUp::Reported)
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
    fn unevaluated(&mut self, expr: ExprId, why: &Unevaluated) {
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

    /// Reports `name`, used at `expr`, when its visibility `visible` does not reach there.
    pub(super) fn check_visible(&mut self, expr: ExprId, name: Symbol, visible: &Visible) {
        if self.env().sees(visible, expr) != Some(false) {
            return;
        }
        self.diag.error(self.env().not_visible(self.hir().exprs[expr].origin, name, visible));
    }

    /// Reports the operator at `expr` as having no impl of its interface `interface` for `ty`,
    /// the type of its left operand.
    pub(super) fn no_impl(&mut self, expr: ExprId, interface: Item, ty: Ty) {
        let msg = Msg::new().text("no impl of ").code(interface.to_string()).text(" for ").ty(ty.to_string());
        let help = Msg::new()
            .text("declare one: a struct of its methods, with the attribute ")
            .code(vun_items::lang::r#impl.to_string())
            .text(" naming ")
            .code(interface.to_string())
            .text(" of ")
            .ty(ty.to_string());
        let d =
            Diagnostic::error(errors::TYPES_NO_IMPL, msg).at(self.hir().exprs[expr].origin, None).help(help);
        self.diag.error(d);
    }

    /// Reports `ty`, of the operand `expr` of a form driven by `interface`, as having no impl of
    /// it; a type whose impls its requirements give is left to them.
    pub(super) fn unimplemented(&mut self, expr: ExprId, interface: Option<Item>, ty: Ty) {
        if let Some(interface) = interface
            && !ty.through_borrows().is_required()
        {
            self.no_impl(expr, interface, ty);
        }
    }

    /// Reports `name`, used at `at` on the type `ty`, as a field, which values have.
    pub(super) fn field_of_type(&mut self, at: OriginId, ty: Ty, name: Symbol) {
        let msg = Msg::new().text("no member ").code(name.as_str()).text(" in the type ").ty(ty.to_string());
        let note = Msg::new().text("it is a field, which a value of the type has");
        self.diag
            .error(Diagnostic::error(errors::TYPES_NO_MEMBER, msg).at(at, None).child(ChildKind::Note, note));
    }

    /// Reports `name`, used at `at`, as no member of `ty`.
    pub(super) fn no_member(&mut self, at: OriginId, ty: Ty, name: Symbol) {
        let msg = Msg::new().text("no member ").code(name.as_str()).text(" in ").ty(ty.to_string());
        let mut d = Diagnostic::error(errors::TYPES_NO_MEMBER, msg).at(at, None);
        if let Some(members) = self.env().cx.get::<Members>(&ty.through_borrows()) {
            let fields = members.fields.iter().map(|f| f.name);
            let decls = members.decls.iter().map(|m| m.name);
            let names: Vec<Symbol> =
                fields.chain(decls).chain(members.variants.iter().map(|v| v.name)).collect();
            if let Some(near) = closest(name.as_str(), names.iter().map(|n| n.as_str())) {
                d = d.help(Msg::did_you_mean(near));
            }
        }
        self.diag.error(d);
    }
}
