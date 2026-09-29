//! Calls as written (spec 01, section 4): the callee, a method's receiver and the arguments, applied
//! as every call is (`apply`). A signature whose types name `$` parameters, such as
//! `($T: type, x: u8) -> T`, is typed per call, inside the callee with those parameters bound to
//! the call's arguments.

use vun_diag::{ChildKind, DiagCtxt, Diagnostic, Msg, TraceFrame, errors};
use vun_hir::tree::{Arg, Entry, ExprId, ExprKind};
use vun_hir::visit::Visitor;
use vun_origin::OriginTable;
use vun_query::Cx;
use vun_sema_ty::Ty;
use vun_sema_ty::call::Instantiation;

use super::apply::{Argument, Callee, Passing};
use super::omitted::{Plan, Unifier};
use super::{Known, Typer};
use crate::callable::Callable;
use crate::callable::Parameters;
use crate::member::MemberValue;

impl Typer<'_, '_, '_> {
    /// A call of a value of function type: its arguments are checked against the parameters, and
    /// it has the result type. `$` arguments left out are inferred from the others' types.
    pub(super) fn call(&mut self, call: ExprId, callee: ExprId, args: &[Entry<Arg>]) -> Option<Known> {
        let signature = self.typed(callee).and_then(|k| k.ty.callable());
        let written: Option<Vec<ExprId>> = args
            .iter()
            .map(|a| match a {
                Entry::One(Arg { name: None, value }) => Some(*value),
                _ => None,
            })
            .collect();
        // Named and generated arguments are typed; no call takes them yet.
        let Some(written) = written else {
            let hir = self.hir();
            for arg in args {
                match arg {
                    Entry::One(Arg { value, .. }) => {
                        self.typed(*value);
                    }
                    Entry::Staged(staged) => self.staged(hir, staged, &mut |v, a: &Arg| {
                        v.typed(a.value);
                    }),
                }
            }
            return None;
        };
        // A method's receiver is passed in the mode its first parameter asks for.
        let receiver = match &self.hir().exprs[callee].kind {
            ExprKind::Member { object, .. } if self.found.methods.contains_key(&callee) => Some(*object),
            _ => None,
        };
        let target = Callee { f: self.called(callee), signature, receiver, passing: Passing::Written };
        let written: Vec<Argument> = written.into_iter().map(Argument::Expr).collect();
        let applied = self.apply(call, &target, &written)?;
        if let Some(inferred) = applied.inferred {
            self.finding().inferred.insert(call, inferred);
        }
        if let Some(pack) = applied.pack {
            self.finding().packs.insert(call, pack);
        }
        Some(applied.known)
    }

    /// Reports the `$` arguments `plan` leaves out of a call of `f` as not inferred from the
    /// types of its `operands`, when each has one or is typed by its context.
    pub(super) fn cannot_infer(
        &mut self,
        call: ExprId,
        f: &Callable,
        plan: &Plan,
        types: &[Option<Ty>],
        operands: &[Option<ExprId>],
    ) {
        // A form typed by its context, such as `.none`, has no type to infer from.
        let hir = self.hir();
        let contextual = |e: Option<ExprId>| match e.map(|e| &hir.exprs[e].kind) {
            Some(ExprKind::Call { callee, .. }) => matches!(hir.exprs[*callee].kind, ExprKind::DotName(_)),
            _ => false,
        };
        if !types.iter().zip(operands).all(|(ty, &e)| ty.is_some() || contextual(e)) {
            return;
        }
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
        let help =
            Msg::new().text("write every such ").code("$").text(" argument, in order, before the others");
        self.diag.error(
            Diagnostic::error(errors::TYPES_CANNOT_INFER, msg)
                .at(self.hir().exprs[call].origin, None)
                .child(ChildKind::Note, note)
                .child(ChildKind::Help, help),
        );
    }

    /// Reports a call passing `found` arguments to a function taking one of `counts`, each
    /// counting the receiver, which the message leaves out.
    pub(super) fn arity(&mut self, call: ExprId, counts: &[usize], receiver: usize, found: usize) {
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

    /// What a call of `callee` runs, when known here: a method, or a function or primitive
    /// known at compile time.
    fn called(&self, callee: ExprId) -> Option<Callable> {
        let f = match self.found.methods.get(&callee) {
            Some(method) => self.env().cx.get::<MemberValue>(method),
            None => self.evaluating().eval(callee),
        };
        Callable::of(f.ok()?)
    }
}

impl Typer<'_, '_, '_> {
    /// Takes the reports of the instantiation `i` that `call` reaches, each traced through the
    /// call. A library's report that points into the function reporting it is shown at the call,
    /// which led to it (spec 10, section 3).
    pub(super) fn absorb(&mut self, call: ExprId, i: &Instantiation, reports: &DiagCtxt) {
        let origin = self.hir().exprs[call].origin;
        let origins = OriginTable::global();
        let callee = origins.source_span(i.f.literal.expr.origin);
        let mut placed = DiagCtxt::new();
        for d in reports.sorted(origins) {
            // A report placed in the callee's own code moves to the call; one placed in the
            // caller's, as a part of an argument it wrote, stays there.
            let inside = d.primary.as_ref().is_some_and(|p| {
                let at = origins.source_span(p.origin);
                at.file == callee.file && callee.range.contains(at.range)
            });
            placed.report(if d.id == errors::COMPTIME_REPORTED && inside {
                d.clone().moved(origin, Msg::new().text("reported here"))
            } else {
                d.clone()
            });
        }
        let frame = TraceFrame { msg: "in the instantiation this call reaches".into(), origin };
        self.diag.absorb_through(&placed, &frame);
    }
}

/// An instantiation as generic code or not.
pub(super) trait Generic {
    /// Whether this is an instantiation of generic code: a function built in a compile-time
    /// call, or one with `$` parameters, all bound.
    fn generic(&self, cx: &Cx<'_>) -> bool;
}

impl Generic for Instantiation {
    fn generic(&self, cx: &Cx<'_>) -> bool {
        let Some(comptime) = self.f.comptime(cx) else { return false };
        let bound = comptime.iter().zip(&self.args).all(|(&c, arg)| !c || arg.is_some());
        (self.f.within.is_some() || comptime.contains(&true)) && bound
    }
}
