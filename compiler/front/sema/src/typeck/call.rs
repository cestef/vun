//! Calls (spec 01, section 4): arguments are checked against the parameters of the callee's
//! signature. A signature whose types name `$` parameters, such as `($T: type, x: u8) -> T`, is
//! typed per call, inside the callee with those parameters bound to the call's arguments.

use overt_diag::{ChildKind, Diagnostic, Msg, TraceFrame, errors};
use overt_hir::tree::{Arg, Entry, ExprId, ExprKind};
use overt_hir::visit::Visitor;
use overt_query::Cx;

use super::omitted::{Plan, Unifier};
use super::{Known, Typed, Typer};
use crate::env::Env;
use crate::eval::{Func, Instantiation};
use crate::member::MemberValue;
use crate::ty::{Signature, Ty, Value};

impl Typer<'_, '_, '_> {
    /// A call of a value of function type: its arguments are checked against the parameters, and
    /// it has the result type. `$` arguments left out are inferred from the others' types.
    pub(super) fn call(&mut self, call: ExprId, callee: ExprId, args: &[Entry<Arg>]) -> Option<Known> {
        let known = self.typed(callee).and_then(|k| k.ty.callable());
        // A method's receiver is passed in the mode its first parameter asks for.
        let receiver = usize::from(self.methods.contains_key(&callee));
        let f = self.called(callee);
        let written: Option<Vec<ExprId>> = args
            .iter()
            .map(|a| match a {
                Entry::One(Arg { name: None, value }) => Some(*value),
                _ => None,
            })
            .collect();
        let plan =
            f.as_ref().zip(written.as_ref()).and_then(|(f, w)| Plan::of(self.env(), f, receiver + w.len()));
        if let (Some(f), Some(written), None) = (&f, &written, &plan) {
            let counts = Plan::counts(self.env(), f);
            self.arity(call, &counts, receiver, receiver + written.len());
            for &arg in written {
                self.typed(arg);
            }
            return None;
        }
        let slots = plan.as_ref().map_or_else(|| (0..receiver + args.len()).collect(), |p| p.slots.clone());
        let omitted = plan.as_ref().filter(|p| !p.omitted.is_empty());
        let inferred = match (omitted, f.as_ref(), written.as_ref()) {
            (Some(plan), Some(f), Some(written)) => Some(self.inferred(call, callee, f, plan, written)?),
            _ => None,
        };
        let instantiation = f.map(|f| self.instantiation(f, &slots, args, inferred.as_deref()));
        if let Some(i) = instantiation.as_ref().filter(|i| i.generic(self.env().cx))
            && let Some(typed) = self.env().cx.get::<Typed>(i)
        {
            let frame = TraceFrame {
                msg: "in the instantiation this call reaches".into(),
                origin: self.hir().exprs[call].origin,
            };
            self.diag.absorb_through(&typed.diag, &frame);
        }
        let signature = known
            .filter(|_| inferred.is_none())
            .or_else(|| instantiation?.signature(self.env().cx))
            .or_else(|| self.primitive_signature(callee, args));
        let hir = self.hir();
        let (mut found, mut positional) = (Vec::new(), true);
        for arg in args {
            match arg {
                Entry::One(Arg { name: None, value }) => {
                    // An argument for a `Code(T)` parameter is code of a `T`.
                    let param = slots
                        .get(receiver + found.len())
                        .and_then(|&slot| signature.as_ref()?.params.get(slot).copied())
                        .map(|p| p.quoted().unwrap_or(p));
                    found.push((*value, self.typed_as(*value, param)));
                }
                Entry::One(Arg { value, .. }) => {
                    positional = false;
                    self.typed(*value);
                }
                Entry::Staged(staged) => {
                    positional = false;
                    self.staged(hir, staged, &mut |v, a: &Arg| {
                        v.typed(a.value);
                    });
                }
            }
        }
        let signature = signature.filter(|_| positional)?;
        if slots.len() != receiver + found.len() || signature.params.len() < slots.len() {
            return None;
        }
        if inferred.is_none() && signature.params.len() != slots.len() {
            self.arity(call, &[signature.params.len()], receiver, slots.len());
            return None;
        }
        for (&slot, (value, known)) in slots[receiver..].iter().zip(found) {
            if let Some(known) = known {
                let param = signature.params[slot];
                let param = param.quoted().unwrap_or(param);
                self.expect(self.hir().exprs[value].origin, known, param, None);
            }
        }
        Some(Known { ty: signature.result, stage: self.stage })
    }

    /// The values of the `$` parameters `plan` leaves out, from the types of the receiver and the
    /// `written` arguments, recorded for the call; `None`, reported, when one is not determined.
    fn inferred(
        &mut self,
        call: ExprId,
        callee: ExprId,
        f: &Func,
        plan: &Plan,
        written: &[ExprId],
    ) -> Option<Vec<(usize, Value)>> {
        let receiver = match &self.hir().exprs[callee].kind {
            ExprKind::Member { object, .. } if self.methods.contains_key(&callee) => Some(*object),
            _ => None,
        };
        let types: Vec<Option<Ty>> =
            receiver.iter().chain(written).map(|&e| self.typed(e).map(|k| k.ty)).collect();
        let values = Unifier::infer(self.env(), f, plan, &types)?;
        if values.iter().any(Option::is_none) {
            // A form typed by its context, such as `.none`, has no type to infer from.
            let hir = self.hir();
            let contextual = |e: ExprId| match &hir.exprs[e].kind {
                ExprKind::DotName(_) | ExprKind::StructLit { ty: None, .. } => true,
                ExprKind::Call { callee, .. } => matches!(hir.exprs[*callee].kind, ExprKind::DotName(_)),
                _ => false,
            };
            let args = receiver.iter().chain(written);
            if types.iter().zip(args).all(|(ty, &e)| ty.is_some() || contextual(e)) {
                let names = Unifier::names(self.env(), f, plan);
                let msg = Msg::new()
                    .text("cannot infer ")
                    .code_list(names.iter().map(|n| n.as_str()))
                    .text(" from the arguments; write them");
                let note = Msg::new()
                    .text("a ")
                    .code("$")
                    .text(" parameter may be left out when a later parameter's type names it, and is found from the types of the arguments; a form typed by its context, such as ")
                    .code(".none")
                    .text(", gives none");
                let help = Msg::new()
                    .text("write every such ")
                    .code("$")
                    .text(" argument, in order, before the others");
                self.diag.error(
                    Diagnostic::error(errors::TYPES_CANNOT_INFER, msg)
                        .at(self.hir().exprs[call].origin, None)
                        .child(ChildKind::Note, note)
                        .child(ChildKind::Help, help),
                );
            }
            return None;
        }
        let inferred: Vec<(usize, Value)> =
            plan.omitted.iter().copied().zip(values.into_iter().flatten()).collect();
        self.inferred.insert(call, inferred.clone());
        Some(inferred)
    }

    /// Reports a call passing `found` arguments to a function taking one of `counts`, each
    /// counting the receiver, which the message leaves out.
    fn arity(&mut self, call: ExprId, counts: &[usize], receiver: usize, found: usize) {
        let Some(&all) = counts.first() else { return };
        let count = |n: usize| i128::try_from(n.saturating_sub(receiver)).unwrap_or(i128::MAX);
        let mut msg = Msg::new()
            .text("this call passes ")
            .num(count(found))
            .text(" arguments, and the function takes ")
            .num(count(all));
        if let Some(&fewer) = counts.get(1) {
            msg = msg
                .text(", or ")
                .num(count(fewer))
                .text(" leaving out its inferred ")
                .code("$")
                .text(" arguments");
        }
        self.diag.error(Diagnostic::error(errors::TYPES_ARITY, msg).at(self.hir().exprs[call].origin, None));
    }

    /// The function value a call of `callee` runs, when known here: a method, or a function
    /// known at compile time.
    fn called(&self, callee: ExprId) -> Option<Func> {
        let f = match self.methods.get(&callee) {
            Some(method) => self.env().cx.get::<MemberValue>(method),
            None => self.env().eval(callee),
        };
        match f {
            Ok(Value::Fn(f)) => Some(f),
            _ => None,
        }
    }

    /// The instantiation a call of `f` reaches, each `$` parameter bound to the value of its
    /// argument when known here, or to the value inferred for it.
    fn instantiation(
        &self,
        f: Func,
        slots: &[usize],
        args: &[Entry<Arg>],
        inferred: Option<&[(usize, Value)]>,
    ) -> Instantiation {
        let comptime = f.comptime(self.env().cx).unwrap_or_default();
        let quotes = f.quotes(self.env().cx).unwrap_or_default();
        let receiver = slots.len().saturating_sub(args.len());
        let mut values: Vec<Option<Value>> = vec![None; comptime.len()];
        for (&slot, arg) in slots.iter().skip(receiver).zip(args) {
            let (true, Entry::One(Arg { name: None, value })) = (comptime.get(slot) == Some(&true), arg)
            else {
                continue;
            };
            values[slot] = if quotes.get(slot) == Some(&true) {
                Some(Value::Code(self.env().code(*value)))
            } else {
                self.env().eval(*value).ok()
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
    /// The signature of a call of a primitive whose types name its `$` parameters, typed with
    /// them bound to the call's arguments, such as `lang.bounds.first(T, b) -> T`.
    fn primitive_signature(&self, callee: ExprId, args: &[Entry<Arg>]) -> Option<Signature> {
        let Ok(Value::Primitive(primitive)) = self.env().eval(callee) else { return None };
        let values: Vec<_> = args
            .iter()
            .map(|arg| match arg {
                Entry::One(Arg { name: None, value }) => self.env().eval(*value).ok(),
                _ => None,
            })
            .collect();
        self.env().primitive_signature(primitive, &values).ok()
    }
}

impl Instantiation {
    /// Whether this is an instantiation of generic code: a function built in a compile-time
    /// call, or one with `$` parameters, all bound.
    fn generic(&self, cx: &Cx<'_>) -> bool {
        let Some(comptime) = self.f.comptime(cx) else { return false };
        let bound = comptime.iter().zip(&self.args).all(|(&c, arg)| !c || arg.is_some());
        (self.f.within.is_some() || comptime.contains(&true)) && bound
    }

    /// The signature of this instantiation, its types evaluated with its `$` parameters bound.
    fn signature(&self, cx: &Cx<'_>) -> Option<Signature> {
        let inside = Env::inside(cx, &self.f, self.args.iter().cloned())?;
        inside.literal_signature(self.f.literal.expr.id, &self.args).ok()
    }
}
