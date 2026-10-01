//! The `$` parameters of a call bound: those it leaves out inferred from its arguments' types,
//! and each bound to its value in the instantiation the call reaches.

use vun_diag::{ChildKind, Diagnostic, Msg, errors};
use vun_hir::tree::{ExprId, ExprKind};
use vun_primitives::Primitive;
use vun_sema_ty::call::{Func, Instantiation};
use vun_sema_ty::trap::Unevaluated;
use vun_sema_ty::{Signature, Ty, TyKind, Value};

use super::{Argument, Callee, Passing};
use crate::callable::Callable;
use crate::callable::Parameters;
use crate::env::Quoting;
use crate::eval::Instantiate;
use crate::typeck::omitted::{Plan, Unifier};
use crate::typeck::{Stage, Typer};

impl Typer<'_, '_, '_> {
    /// The values of the `$` parameters `plan` leaves out, from the types of the receiver and the
    /// written arguments `args`, as they are passed; `None`, reported, when one is not determined.
    pub(super) fn inferred(
        &mut self,
        call: ExprId,
        f: &Callable,
        plan: &Plan,
        callee: &Callee,
        args: &[Argument],
    ) -> Option<Vec<(usize, Value)>> {
        let expected = self.expected(call);
        let modes = f.modes(self.env()).unwrap_or_default();
        let quotes = f.quotes(self.env()).unwrap_or_default();
        let receiver = callee.receiver.map(|r| (r, Passing::Asked));
        let receiver = receiver.map(|(r, passing)| (Argument::Expr(r), passing));
        let arguments: Vec<_> =
            receiver.into_iter().chain(args.iter().map(|&a| (a, callee.passing))).collect();
        // A function written here: a closure, or a literal without captures.
        let is_written = |this: &Self, arg: &Argument| matches!(arg, Argument::Expr(e) if matches!(this.hir().exprs[*e].kind, ExprKind::Closure { .. } | ExprKind::Fn { .. }));
        // A closure written as an argument is typed last: what the other arguments say of the `$`
        // parameters it is passed for is what its own parameters are.
        let mut types: Vec<Option<Ty>> = vec![None; arguments.len()];
        for closures in [false, true] {
            let inferred =
                if closures { Unifier::infer(self.env(), f, plan, &types, expected) } else { None };
            for (i, ((arg, passing), &slot)) in arguments.iter().zip(&plan.slots).enumerate() {
                if is_written(self, arg) != closures {
                    continue;
                }
                let ty = match *arg {
                    Argument::Expr(arg) => {
                        if closures {
                            let bound =
                                Self::bound_by_slot(plan, inferred.as_deref(), f.comptime(self.env()));
                            if let Some(context) = self.argument_context(f, slot, bound) {
                                self.hints.insert(arg, context);
                            }
                        }
                        self.typed(arg).map(|known| known.ty)
                    }
                    Argument::Matched(ty) => Some(ty),
                };
                types[i] = ty.and_then(|ty| match passing {
                    Passing::Written => Some(ty),
                    Passing::Asked => {
                        let reached = ty.through_borrows();
                        match modes.get(slot).copied().flatten() {
                            Some(mode) => TyKind::borrowed_as(mode, reached),
                            None => Some(reached),
                        }
                    }
                });
                // An argument for a `Code(T)` parameter is passed as code of its type.
                if quotes.get(slot) == Some(&true) {
                    types[i] = types[i].map(TyKind::code);
                }
            }
        }
        let values = Unifier::infer(self.env(), f, plan, &types, expected)?;
        if values.iter().any(Option::is_none) {
            let written = args.iter().map(|a| match a {
                Argument::Expr(e) => Some(*e),
                Argument::Matched(_) => None,
            });
            let operands: Vec<Option<ExprId>> =
                callee.receiver.map(Some).into_iter().chain(written).collect();
            self.cannot_infer(call, f, plan, &types, &operands);
            return None;
        }
        Some(plan.omitted.iter().copied().zip(values.into_iter().flatten()).collect())
    }

    /// The values of the `$` parameters inferred so far, by the position of the parameter.
    fn bound_by_slot(
        plan: &Plan,
        inferred: Option<&[Option<Value>]>,
        comptime: Option<Vec<bool>>,
    ) -> Vec<Option<Value>> {
        let mut bound = vec![None; comptime.map_or(0, |c| c.len())];
        for (&slot, value) in plan.omitted.iter().zip(inferred.into_iter().flatten()) {
            // The type of a literal, `Int(0..=0)` for `0`, is one the call may still widen.
            let exact = value.as_ref().and_then(Value::as_type).and_then(|ty| ty.interval());
            if let Some(entry) = bound.get_mut(slot)
                && exact.is_none_or(|i| i.lo() != i.hi())
            {
                entry.clone_from(value);
            }
        }
        bound
    }

    /// The instantiation a call of `f` reaches, each `$` parameter bound to the value of its
    /// written argument among `args`, at `slots`, when known here, or to the value inferred.
    pub(super) fn instantiation(
        &mut self,
        f: Func,
        slots: &[usize],
        args: &[Argument],
        inferred: Option<&[(usize, Value)]>,
    ) -> Instantiation {
        let comptime = f.comptime(self.env().cx).unwrap_or_default();
        let quotes = f.quotes(self.env().cx).unwrap_or_default();
        // A `$` argument is a value of its parameter's type, as `Env::passing` passes it: `.empty`
        // for `$s: Shape` is the variant.
        let params = Callable::Fn(f.clone()).types(self.env()).unwrap_or_default();
        let mut values: Vec<Option<Value>> = vec![None; comptime.len()];
        for (&slot, &arg) in slots.iter().zip(args) {
            let (Some(true), Argument::Expr(arg)) = (comptime.get(slot), arg) else { continue };
            values[slot] = if quotes.get(slot) == Some(&true) {
                Some(Value::Code(self.env().code(arg)))
            } else {
                self.known_value(arg, params.get(slot).copied().flatten()).ok()
            };
        }
        for (slot, value) in inferred.into_iter().flatten() {
            if let Some(v) = values.get_mut(*slot) {
                *v = Some(value.clone());
            }
        }
        Instantiation { args: values, f }
    }

    /// The signature of the instantiation `instantiation` that the call at `call` reaches. A trap
    /// in it is reported with the rest of the instantiation's when `absorbed`; why else it has no
    /// signature is counted, as reading an unbound `$` parameter is.
    pub(super) fn signature_of(
        &mut self,
        call: ExprId,
        instantiation: &Instantiation,
        absorbed: bool,
    ) -> Option<Signature> {
        let signature = instantiation.signature(self.env().cx);
        if absorbed {
            signature.map_err(|why| self.unevaluated(call, &why)).ok()
        } else {
            self.reported(call, signature).ok()
        }
    }
}

impl Typer<'_, '_, '_> {
    /// Reports the call `expr` of `receiver` and `args` whose result's range depends on a value
    /// known only when it runs, such as a constant shifted by a run-time amount: where an
    /// operand is computed at run time, the intermediate has no width (spec 04, rule 5).
    pub(super) fn unbounded(&mut self, expr: ExprId, receiver: Option<ExprId>, args: &[Argument]) {
        let written = receiver.map(Argument::Expr).into_iter().chain(args.iter().copied());
        let run = written.into_iter().any(|a| match a {
            Argument::Expr(e) => self.found.types.get(&e).is_some_and(|k| k.stage == Stage::Run),
            Argument::Matched(_) => false,
        });
        if !run {
            return;
        }
        let msg = Msg::new().text("this intermediate has no width: its range depends on a run-time value");
        let help = Msg::new()
            .text("bring a constant operand into a ")
            .code("uN")
            .text(" type with ")
            .code(".to(uN)")
            .text(", or narrow the other operand");
        let d = Diagnostic::error(errors::INT_WIDTH_CAP, msg)
            .at(self.hir().exprs[expr].origin, None)
            .child(ChildKind::Help, help);
        self.diag.error(d);
    }

    /// The signature of the primitive `p` for a call passing `receiver` and `args` at `slots`,
    /// with its `$` parameters bound to their arguments' values known here, or to those
    /// inferred.
    pub(super) fn primitive_signature(
        &mut self,
        p: Primitive,
        receiver: Option<ExprId>,
        slots: &[usize],
        args: &[Argument],
        inferred: Option<&[(usize, Value)]>,
    ) -> Result<Signature, Unevaluated> {
        let written = receiver.map(Argument::Expr).into_iter().chain(args.iter().copied());
        let comptime = Callable::Primitive(p).comptime(self.env()).unwrap_or_default();
        let mut values: Vec<Option<Value>> = vec![None; slots.len() + inferred.map_or(0, <[_]>::len)];
        for (&slot, arg) in slots.iter().zip(written) {
            let (Some(true), Argument::Expr(e)) = (comptime.get(slot), arg) else { continue };
            let value = self.known_value(e, None).ok();
            if let Some(v) = values.get_mut(slot) {
                *v = value;
            }
        }
        for (slot, value) in inferred.into_iter().flatten() {
            if let Some(v) = values.get_mut(*slot) {
                *v = Some(value.clone());
            }
        }
        self.env().primitive_signature(p, &values)
    }
}
