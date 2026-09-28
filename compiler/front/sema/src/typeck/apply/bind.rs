//! The `$` parameters of a call bound: those it leaves out inferred from its arguments' types,
//! and each bound to its value in the instantiation the call reaches.

use vun_hir::tree::ExprId;

use super::{Argument, Callee, Passing};
use crate::Primitive;
use crate::callable::Callable;
use crate::env::Unevaluated;
use crate::eval::{Func, Instantiation};
use crate::ty::{Signature, Ty, TyKind, Value};
use crate::typeck::Typer;
use crate::typeck::omitted::{Plan, Unifier};

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
        let modes = f.modes(self.env()).unwrap_or_default();
        let quotes = f.quotes(self.env()).unwrap_or_default();
        let receiver = callee.receiver.map(|r| (r, Passing::Asked));
        let receiver = receiver.map(|(r, passing)| (Argument::Expr(r), passing));
        let arguments = receiver.into_iter().chain(args.iter().map(|&a| (a, callee.passing)));
        let types: Vec<Option<Ty>> = arguments
            .zip(&plan.slots)
            .map(|((arg, passing), &slot)| {
                let ty = match arg {
                    Argument::Expr(arg) => self.typed(arg)?.ty,
                    Argument::Matched(ty) => ty,
                };
                let ty = match passing {
                    Passing::Written => Some(ty),
                    Passing::Asked => {
                        let reached = ty.through_borrows();
                        match modes.get(slot).copied().flatten() {
                            Some(mode) => TyKind::borrowed_as(mode, reached),
                            None => Some(reached),
                        }
                    }
                };
                // An argument for a `Code(T)` parameter is passed as code of its type.
                if quotes.get(slot) == Some(&true) { ty.map(TyKind::code) } else { ty }
            })
            .collect();
        let values = Unifier::infer(self.env(), f, plan, &types)?;
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

    /// The instantiation a call of `f` reaches, each `$` parameter bound to the value of its
    /// written argument among `args`, at `slots`, when known here, or to the value inferred.
    pub(super) fn instantiation(
        &self,
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
                self.env().eval_as(arg, params.get(slot).copied().flatten()).ok()
            };
        }
        for (slot, value) in inferred.into_iter().flatten() {
            if let Some(v) = values.get_mut(*slot) {
                *v = Some(value.clone());
            }
        }
        Instantiation { args: values, f }
    }
}

impl Typer<'_, '_, '_> {
    /// The signature of the primitive `p` for a call passing `receiver` and `args` at `slots`,
    /// with its `$` parameters bound to their arguments' values known here, or to those
    /// inferred.
    pub(super) fn primitive_signature(
        &self,
        p: Primitive,
        receiver: Option<ExprId>,
        slots: &[usize],
        args: &[Argument],
        inferred: Option<&[(usize, Value)]>,
    ) -> Result<Signature, Unevaluated> {
        let written = receiver.map(Argument::Expr).into_iter().chain(args.iter().copied());
        let mut values: Vec<Option<Value>> = vec![None; slots.len() + inferred.map_or(0, <[_]>::len)];
        for (&slot, arg) in slots.iter().zip(written) {
            if let (Some(v), Argument::Expr(e)) = (values.get_mut(slot), arg) {
                *v = self.env().eval(e).ok();
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
