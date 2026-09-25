//! Applying a function to arguments (spec 01, section 4), what calls and operators share: the
//! arguments are checked against the parameters of the callee's signature, which is typed per
//! call when its types name `$` parameters, those left out inferred from the others' types.

use vun_hir::tree::ExprId;

use super::omitted::{Plan, Unifier};
use super::{Known, Typed, Typer};
use crate::Primitive;
use crate::callable::Callable;
use crate::env::Unevaluated;
use crate::eval::{Func, Instantiation};
use crate::ty::{Signature, Ty, TyKind, Value};
use vun_diag::Msg;

/// How a call's arguments are passed.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Passing {
    /// As written: each argument is its parameter's value.
    Written,
    /// As its parameter asks, an operator's operands (appendix B): each reached through its
    /// borrows, as `.` reaches a value, then borrowed in the mode its parameter is written with,
    /// or read.
    Asked,
}

/// An argument of a call: an expression, or the value a pattern matches, which has a type and no
/// expression of its own.
#[derive(Clone, Copy)]
pub(super) enum Argument {
    Expr(ExprId),
    Matched(Ty),
}

/// What a call calls.
pub(super) struct Callee {
    /// The function or primitive it runs, when known here.
    pub(super) f: Option<Callable>,
    /// The type of the value called, when it is of function type.
    pub(super) signature: Option<Signature>,
    /// The operand found by lookup rather than written among the arguments, passed first as its
    /// parameter asks: the receiver `a` of `a.f(b)`, or an operator's left operand.
    pub(super) receiver: Option<ExprId>,
    pub(super) passing: Passing,
}

/// A call typed: its value, and the `$` arguments inferred for it.
pub(super) struct Applied {
    pub(super) known: Known,
    pub(super) inferred: Option<Vec<(usize, Value)>>,
}

impl Typer<'_, '_, '_> {
    /// The call at `call` of `callee` with the written arguments `args`, typed. `$` arguments left
    /// out are inferred from the other arguments' types.
    pub(super) fn apply(&mut self, call: ExprId, callee: &Callee, args: &[Argument]) -> Option<Applied> {
        let receiver = usize::from(callee.receiver.is_some());
        let count = receiver + args.len();
        let plan = callee.f.as_ref().and_then(|f| Plan::of(self.env(), f, count));
        if let (Some(f), None) = (&callee.f, &plan) {
            let counts = Plan::counts(self.env(), f);
            self.arity(call, &counts, receiver, count);
            for &arg in args {
                if let Argument::Expr(arg) = arg {
                    self.typed(arg);
                }
            }
            return None;
        }
        let slots = plan.as_ref().map_or_else(|| (0..count).collect(), |p| p.slots.clone());
        let omitted = plan.as_ref().filter(|p| !p.omitted.is_empty());
        let inferred = match (omitted, &callee.f) {
            (Some(plan), Some(f)) => Some(self.inferred(call, f, plan, callee, args)?),
            _ => None,
        };
        let instantiation = match &callee.f {
            Some(Callable::Fn(f)) => {
                Some(self.instantiation(f.clone(), &slots[receiver..], args, inferred.as_deref()))
            }
            _ => None,
        };
        // A generic instantiation's typing reports a trap in its own signature, which the call
        // takes with the rest of its reports.
        let absorbed = instantiation.as_ref().filter(|i| i.generic(self.env().cx)).and_then(|i| {
            let typed = self.env().cx.get::<Typed>(i)?;
            self.absorb(call, i, &typed.diag);
            Some(())
        });
        let signature = callee
            .signature
            .clone()
            .filter(|_| inferred.is_none())
            .or_else(|| {
                let signature = instantiation?.signature(self.env().cx);
                if absorbed.is_some() { signature.ok() } else { self.reported(call, signature).ok() }
            })
            .or_else(|| {
                let Some(Callable::Primitive(p)) = callee.f else { return None };
                let signature =
                    self.primitive_signature(p, callee.receiver, &slots, args, inferred.as_deref());
                self.reported(call, signature).ok()
            });
        let mut found = Vec::new();
        for (&slot, &arg) in slots[receiver..].iter().zip(args) {
            // An argument for a `Code(T)` parameter is code of a `T`.
            let param = signature.as_ref().and_then(|s| s.params.get(slot).copied());
            let param = param.map(|p| callee.passing.expected(p.quoted().unwrap_or(p)));
            found.push(match arg {
                Argument::Expr(expr) => (expr, self.typed_as(expr, param)),
                Argument::Matched(ty) => (call, Some(Known { ty, stage: self.stage })),
            });
        }
        let signature = signature?;
        if signature.is_unsafe() {
            self.unchecked(call, Msg::new().text("calling a function whose type is ").code("unsafe"));
        }
        if slots.len() != count || signature.params.len() < slots.len() {
            return None;
        }
        if inferred.is_none() && signature.params.len() != slots.len() {
            self.arity(call, &[signature.params.len()], receiver, slots.len());
            return None;
        }
        for (&slot, (value, known)) in slots[receiver..].iter().zip(found) {
            if let Some(known) = known {
                let param = signature.params[slot];
                let param = callee.passing.expected(param.quoted().unwrap_or(param));
                let known = Known { ty: callee.passing.passed(known.ty, param), ..known };
                self.expect(self.hir().exprs[value].origin, known, param, None);
            }
        }
        Some(Applied { known: Known { ty: signature.result, stage: self.stage }, inferred })
    }

    /// The values of the `$` parameters `plan` leaves out, from the types of the receiver and the
    /// written arguments `args`, as they are passed; `None`, reported, when one is not determined.
    fn inferred(
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
    fn instantiation(
        &self,
        f: Func,
        slots: &[usize],
        args: &[Argument],
        inferred: Option<&[(usize, Value)]>,
    ) -> Instantiation {
        let comptime = f.comptime(self.env().cx).unwrap_or_default();
        let quotes = f.quotes(self.env().cx).unwrap_or_default();
        let mut values: Vec<Option<Value>> = vec![None; comptime.len()];
        for (&slot, &arg) in slots.iter().zip(args) {
            let (Some(true), Argument::Expr(arg)) = (comptime.get(slot), arg) else { continue };
            values[slot] = if quotes.get(slot) == Some(&true) {
                Some(Value::Code(self.env().code(arg)))
            } else {
                self.env().eval(arg).ok()
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
    fn primitive_signature(
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

impl Passing {
    /// The type an argument is expected to have for a parameter of type `param`: the type it
    /// borrows, for an operand passed as its parameter asks.
    fn expected(self, param: Ty) -> Ty {
        match self {
            Self::Written => param,
            Self::Asked => param.borrowed().unwrap_or(param),
        }
    }

    /// The type an argument of type `ty` is passed as where `expected` is expected of it: an
    /// operand passed as asked is reached through its borrows, and an array stands for the slice
    /// of its items where a slice is borrowed, as in any call.
    fn passed(self, ty: Ty, expected: Ty) -> Ty {
        match self {
            Self::Written => ty,
            Self::Asked => {
                let reached = ty.through_borrows();
                match (expected.slice_items(), reached.as_array()) {
                    (Some(_), Some((_, item))) => TyKind::slice(item),
                    _ => reached,
                }
            }
        }
    }
}
