//! Applying a function to arguments (spec 01, section 4), what calls and operators share: the
//! arguments are checked against the parameters of the callee's signature, which is typed per
//! call when its types name `$` parameters, those left out inferred from the others' types.

use vun_codec::{Decode, Encode};
use vun_hir::tree::ExprId;

mod bind;

use super::omitted::Plan;
use super::{Known, Reported, Stage, Typer};
use crate::callable::Callable;
use crate::int::Interval;
use crate::ty::{Signature, Ty, TyKind, Value};
use vun_diag::Msg;

/// How a call's arguments are passed.
#[derive(Clone, Copy, PartialEq, Eq, Encode, Decode)]
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

/// A call typed: its value, the `$` arguments inferred for it, and where the written arguments
/// its callee's pack gathers start.
pub(super) struct Applied {
    pub(super) known: Known,
    pub(super) inferred: Option<Vec<(usize, Value)>>,
    pub(super) pack: Option<usize>,
}

impl Typer<'_, '_, '_> {
    /// The call at `call` of `callee` with the written arguments `args`, typed. `$` arguments left
    /// out are inferred from the other arguments' types.
    pub(super) fn apply(&mut self, call: ExprId, callee: &Callee, args: &[Argument]) -> Option<Applied> {
        let receiver = usize::from(callee.receiver.is_some());
        let count = receiver + args.len();
        let plan = match &callee.f {
            Some(f) => Plan::of(self.env(), f, count),
            None => callee.signature.as_ref().and_then(|s| Plan::typed(&s.params, count)),
        };
        // Without a plan the call passes a count the function does not take, or its literal is
        // not read here, and the signature alone checks it.
        let counts = callee.f.as_ref().filter(|_| plan.is_none()).map(|f| Plan::counts(self.env(), f));
        if let Some(counts) = counts.filter(|c| !c.is_empty()) {
            self.arity(call, &counts, receiver, count);
            for &arg in args {
                if let Argument::Expr(arg) = arg {
                    self.typed(arg);
                }
            }
            return None;
        }
        let slots = plan.as_ref().map_or_else(|| (0..count).collect(), |p| p.slots.clone());
        let items = plan.as_ref().map_or_else(|| vec![None; count], Plan::items);
        let pack = plan.as_ref().and_then(|p| p.pack);
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
            let reported = self.env().cx.get::<Reported>(i)?;
            self.absorb(call, i, &reported);
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
        for ((&slot, &item), &arg) in slots[receiver..].iter().zip(&items[receiver..]).zip(args) {
            // An argument for a `Code(T)` parameter is code of a `T`.
            let param = signature.as_ref().and_then(|s| Self::param(s, slot, item));
            let param = param.map(|p| callee.passing.expected(p.quoted().unwrap_or(p)));
            found.push(match arg {
                Argument::Expr(expr) => (expr, self.retyped(expr, param, inferred.is_some())),
                Argument::Matched(ty) => (call, Some(Known { ty, stage: self.stage })),
            });
        }
        let signature = signature?;
        if signature.is_unsafe() {
            self.unchecked(call, Msg::new().text("calling a function whose type is ").code("unsafe"));
        }
        // The parameters the call reaches: one per argument, but one for its pack's.
        let reached = slots.iter().filter(|&&s| Some(s) != pack).count() + usize::from(pack.is_some());
        if slots.len() != count || signature.params.len() < reached {
            return None;
        }
        let gathered = pack.and_then(|p| signature.params[p].packed()?.as_tuple().map(<[Ty]>::len));
        let passed = slots.iter().filter(|&&s| Some(s) == pack).count();
        if inferred.is_none() && signature.params.len() != reached || gathered.is_some_and(|n| n != passed) {
            let takes = signature.params.len() - usize::from(pack.is_some()) + gathered.unwrap_or(0);
            self.arity(call, &[takes], receiver, count);
            return None;
        }
        for ((&slot, &item), (value, known)) in slots[receiver..].iter().zip(&items[receiver..]).zip(found) {
            if let Some(known) = known
                && let Some(param) = Self::param(&signature, slot, item)
            {
                let param = callee.passing.expected(param.quoted().unwrap_or(param));
                let known = Known { ty: callee.passing.passed(known.ty, param), ..known };
                self.expect(self.hir().exprs[value].origin, known, param, None);
            }
        }
        // A call that folds is known when compiling, and an integer it gives has the type of its
        // value alone, as a literal does (spec 04): `size(T) * 8` is `64` for `f64`.
        if callee.f.as_ref().is_some_and(|f| f.folds(self.env())) {
            let ty = match self.env().eval(call) {
                Ok(Value::Int(n)) if signature.result.interval().is_some() => {
                    TyKind::int(&Interval::singleton(n))
                }
                _ => signature.result,
            };
            return Some(Applied { known: Known { ty, stage: Stage::Compile }, inferred, pack: None });
        }
        let pack = pack.map(|p| slots.iter().position(|&s| s == p).unwrap_or(count) - receiver);
        Some(Applied { known: Known { ty: signature.result, stage: self.stage }, inferred, pack })
    }

    /// The type of the parameter at `slot` of `signature` an argument is for: for the `item`th
    /// argument of a pack, that item of the pack's tuple.
    fn param(signature: &Signature, slot: usize, item: Option<usize>) -> Option<Ty> {
        let param = *signature.params.get(slot)?;
        match (item, param.packed()) {
            (Some(item), Some(tuple)) => tuple.as_tuple()?.get(item).copied(),
            _ => Some(param),
        }
    }

    /// `expr` typed as `param`. An argument typed already, to infer the `$` parameters left out,
    /// keeps its type when that is the parameter's: typing it again would give the same, and
    /// nested calls of generic functions would each type their arguments twice over.
    fn retyped(&mut self, expr: ExprId, param: Option<Ty>, typed: bool) -> Option<Known> {
        match self.found.types.get(&expr) {
            Some(&known) if typed && param == Some(known.ty) => Some(known),
            _ => self.typed_as(expr, param),
        }
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
