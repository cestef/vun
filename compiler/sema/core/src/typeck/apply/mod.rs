//! Applying a function to arguments (spec 01, section 4), what calls and operators share: the
//! arguments are checked against the parameters of the callee's signature, which is typed per
//! call when its types name `$` parameters, those left out inferred from the others' types.

use vun_codec::{Decode, Encode};
use vun_hir::tree::ExprId;

mod bind;
mod bounds;

use vun_diag::Msg;
use vun_sema_int::Interval;
use vun_sema_ty::trap::Unevaluated;
use vun_sema_ty::{Signature, Ty, TyKind, Value};

use super::omitted::Plan;
use super::{Known, Reported, Stage, Typer};
use crate::callable::Callable;
use crate::callable::Generic;

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
        if let Some(instantiation) = &instantiation {
            let inferred: Vec<usize> = inferred.iter().flatten().map(|&(slot, _)| slot).collect();
            self.check_bounds(call, instantiation, &slots[receiver..], args, &inferred);
        }
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
            .or_else(|| self.signature_of(call, instantiation.as_ref()?, absorbed.is_some()))
            .or_else(|| {
                let Some(Callable::Primitive(p)) = callee.f else { return None };
                let signature =
                    self.primitive_signature(p, callee.receiver, &slots, args, inferred.as_deref());
                if matches!(signature, Err(Unevaluated::Unbounded)) {
                    self.unbounded(call, callee.receiver, args);
                }
                self.reported(call, signature).ok()
            });
        let at = (&slots[receiver..], &items[receiver..]);
        let found = self.arguments(call, callee, signature.as_ref(), at, args, inferred.is_some());
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
                let param = callee.passing.expected(param.quoted().unwrap_or(param), false);
                let known = Known { ty: callee.passing.passed(known.ty, param), ..known };
                self.expect(self.hir().exprs[value].origin, known, param, None);
            }
        }
        // A call that folds is known when compiling, and an integer it gives has the type of its
        // value alone, as a literal does (spec 04): `size(T) * 8` is `64` for `f64`, and so is a
        // function of `$` parameters computing it.
        if callee.f.as_ref().is_some_and(|f| f.folds(self.env())) {
            let ty = match signature.result.interval() {
                Some(_) => match self.evaluating().eval(call) {
                    Ok(Value::Int(n)) => TyKind::int(&Interval::singleton(n)),
                    _ => signature.result,
                },
                None => signature.result,
            };
            return Some(Applied { known: Known { ty, stage: Stage::Compile }, inferred, pack: None });
        }
        let pack = pack.map(|p| slots.iter().position(|&s| s == p).unwrap_or(count) - receiver);
        // A value that exists at compile time only, such as a type, is known then, whatever
        // the arguments it is computed from: `TypeOf(x)` of a run-time `x`.
        let stage = if self.env().compile_only_type(signature.result) { Stage::Compile } else { self.stage };
        Some(Applied { known: Known { ty: signature.result, stage }, inferred, pack })
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

    /// The written arguments `args`, at the `slots` of the callee's parameters and the `items` of
    /// a pack they are, typed as the parameters of `signature` expect them, each with the
    /// expression it was typed at.
    fn arguments(
        &mut self,
        call: ExprId,
        callee: &Callee,
        signature: Option<&Signature>,
        (slots, items): (&[usize], &[Option<usize>]),
        args: &[Argument],
        inferred: bool,
    ) -> Vec<(ExprId, Option<Known>)> {
        let mut found = Vec::new();
        for ((&slot, &item), &arg) in slots.iter().zip(items).zip(args) {
            // An argument for a `Code(T)` parameter is code of a `T`.
            let param = signature.and_then(|s| Self::param(s, slot, item));
            let borrowed = matches!(arg, Argument::Expr(e) if self.hir().exprs[e].kind.is_borrow());
            let param = param.map(|p| callee.passing.expected(p.quoted().unwrap_or(p), borrowed));
            found.push(match arg {
                Argument::Expr(expr) => {
                    let known = self.retyped(expr, param, inferred);
                    (expr, self.stored(callee.passing, expr, known, param))
                }
                Argument::Matched(ty) => (call, Some(Known { ty, stage: self.stage })),
            });
        }
        found
    }

    /// An operand passed as asked is borrowed, which converts nothing: where it is a temporary,
    /// such as the literal in `xs == [1, 2]`, it is stored as the type expected of it when that
    /// type accepts it, as `borrow` stores the temporary in `&[1, 2]`; a place keeps its own.
    fn stored(
        &mut self,
        passing: Passing,
        expr: ExprId,
        known: Option<Known>,
        param: Option<Ty>,
    ) -> Option<Known> {
        let (Some(known), Some(param)) = (known, param) else { return known };
        // A borrow written as the operand has stored its own operand already.
        let kind = &self.hir().exprs[expr].kind;
        let temporary = !kind.is_place() && !kind.is_borrow();
        if passing != Passing::Asked || !temporary || param == known.ty || !param.accepts(&known.ty) {
            return Some(known);
        }
        let known = Known { ty: param, ..known };
        self.finding().types.insert(expr, known);
        Some(known)
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
    /// borrows, for an operand passed as its parameter asks, unless the operand is written as a
    /// borrow itself (`borrowed`), whose operand then takes the type the parameter borrows.
    fn expected(self, param: Ty, borrowed: bool) -> Ty {
        match self {
            Self::Asked if !borrowed => param.borrowed().unwrap_or(param),
            Self::Written | Self::Asked => param,
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
