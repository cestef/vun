//! Calls: `a.f(b)` is `T.f(a, b)`, with the method the typer found through `T`'s namespace or
//! impls (spec 01, section 4), and an argument for a `Code(T)` parameter is passed as code
//! (spec 03, section 4).

use vun_desugar::{Borrow, ITERATION, IterationRow};
use vun_hir::tree::{Arg, Entry, ExprId, ExprKind, Name};
use vun_intern::Symbol;
use vun_origin::OriginId;
use vun_sema::callable::Callable;
use vun_sema::env::Passed;
use vun_sema::member::{MemberType, MemberValue};
use vun_sema::ty::{Ty, TyKind, Value};

use super::{Builder, Unbuilt};
use crate::place::Place;
use crate::{Aggregate, Operand, Rvalue, TerminatorKind};

/// A method, by the type it is found through and its name: the key of `MemberType`.
type Method = (Ty, Symbol);

/// How a method's receiver is passed: borrowed in a mode, from a place computed before the
/// arguments, or as a value.
enum Receiver {
    Borrowed { mode: Borrow, place: Place, origin: OriginId },
    Value(Operand),
}

impl Builder<'_, '_, '_> {
    pub(super) fn call(
        &mut self,
        call: ExprId,
        callee: ExprId,
        args: &[Entry<Arg>],
        origin: OriginId,
    ) -> Result<Operand, Unbuilt> {
        let unbuilt = Unbuilt { at: origin };
        if let Some(value) = self.frame_call(call, callee, args, origin)? {
            return Ok(value);
        }
        // The `$` arguments the call leaves out, as the typer inferred them.
        let inferred = self.typed().and_then(|t| t.inferred.get(&call).cloned()).unwrap_or_default();
        // `a.f(b)` is `T.f(a, b)`, `f` the method the typer found (spec 01, section 4).
        let method = self.method(callee)?;
        let callee = match &method {
            Some((f, ..)) => f.clone(),
            None => self.operand(callee)?,
        };
        // A variant's constructor builds the variant.
        if let Operand::Const(Value::Constructor { ty, name }) = &callee {
            let kind = Aggregate::Variant { ty: *ty, name: *name };
            let parts = args
                .iter()
                .map(|arg| match arg {
                    Entry::One(Arg { name: None, value }) => self.operand(*value),
                    _ => Err(unbuilt),
                })
                .collect::<Result<_, _>>()?;
            return Ok(self.temp(Rvalue::Aggregate { kind, parts }, origin));
        }
        let known = match &callee {
            Operand::Const(value) => Some(value),
            Operand::Place(_) => None,
        };
        let passing = self.env().passing(known, method.is_some(), args, &inferred).map_err(|_| unbuilt)?;
        let at = passing.iter().position(|p| *p == Passed::Receiver);
        let receiver = match (method, at) {
            (Some((f, receiver, method)), Some(at)) => Some((at, self.receiver(&f, &method, receiver, at)?)),
            _ => None,
        };
        let expansion = match &callee {
            Operand::Const(Value::Fn(f)) if passing.iter().any(|p| matches!(p, Passed::Code(_))) => {
                self.expansion(f, &passing)
            }
            _ => None,
        };
        let mut values = Vec::new();
        for passed in passing {
            let value = self.passed(passed, origin)?;
            values.push(value);
        }
        // A borrowed receiver is borrowed after the arguments are evaluated, its place computed
        // before them: `v.push(v.len())` reads `v` before `&mut v` is taken (two-phase).
        match receiver {
            Some((at, Receiver::Borrowed { mode, place, origin })) => {
                values[at] = self.temp(Rvalue::Ref { mode, place }, origin);
            }
            Some((at, Receiver::Value(operand))) => values[at] = operand,
            None => {}
        }
        let result = match expansion {
            Some(body) => self.expand(body, values, origin)?,
            None => self.temp(Rvalue::Call { callee, args: values }, origin),
        };
        // A call of an empty type, such as `never`, does not come back.
        if self.type_of(call).is_some_and(|t| t.is_empty()) {
            self.terminate(TerminatorKind::Unreachable, origin);
            self.current = self.fresh(origin);
        }
        Ok(result)
    }

    /// What is passed to one parameter (see [`vun_sema::env::Env::passing`]), the receiver's
    /// slot filled later: code and `$` arguments are compile-time values of the call running the
    /// body.
    fn passed(&mut self, passed: Passed, origin: OriginId) -> Result<Operand, Unbuilt> {
        Ok(match passed {
            Passed::Receiver => Self::unit(),
            Passed::Inferred(value) => Operand::Const(value),
            // Code of a `T` is a `Code(T)`.
            Passed::Code(expr) => {
                let quote = Rvalue::Quote { expr, staged: self.stepped() };
                self.typed_temp(quote, self.type_of(expr).map(TyKind::code), origin)
            }
            Passed::Known { expr, ty } => {
                self.typed_temp(self.comptime(expr, ty), self.type_of(expr).or(ty), origin)
            }
            Passed::Run { expr, .. } => self.operand(expr)?,
        })
    }

    /// `frame.resume()` or `frame.end()` on a frame, which answers its interface's `resume` and
    /// `end` itself (the table `iteration`); `None` for any other call.
    fn frame_call(
        &mut self,
        call: ExprId,
        callee: ExprId,
        args: &[Entry<Arg>],
        origin: OriginId,
    ) -> Result<Option<Operand>, Unbuilt> {
        let ExprKind::Member { object, name: Name::Plain(name) } = self.env.parsed.hir.exprs[callee].kind
        else {
            return Ok(None);
        };
        let ty = self.type_of(object);
        let framed = std::iter::successors(ty, |t| t.borrowed()).any(|t| matches!(&*t, TyKind::Frame(_)));
        let Some(&IterationRow { resume, end, .. }) = ITERATION.first() else { return Ok(None) };
        if !framed || !args.is_empty() {
            return Ok(None);
        }
        let rvalue = if name == resume.symbol() {
            let step = self.type_of(call).ok_or(Unbuilt { at: origin })?;
            Rvalue::Resume { frame: self.through_borrows(object)?, step }
        } else if name == end.symbol() {
            Rvalue::End { frame: self.through_borrows(object)? }
        } else {
            return Ok(None);
        };
        Ok(Some(self.temp(rvalue, origin)))
    }

    /// The method `callee` names through a value, the value, and the method by its type and
    /// name; `None` when `callee` names no method.
    fn method(&self, callee: ExprId) -> Result<Option<(Operand, ExprId, Method)>, Unbuilt> {
        let e = &self.env.parsed.hir.exprs[callee];
        let ExprKind::Member { object, .. } = e.kind else { return Ok(None) };
        let unbuilt = Unbuilt { at: e.origin };
        let typed = self.typed().ok_or(unbuilt)?;
        let Some(method) = typed.methods.get(&callee) else { return Ok(None) };
        let f = self.env.cx.get::<MemberValue>(method).map_err(|_| unbuilt)?;
        Ok(Some((Operand::Const(f), object, *method)))
    }

    /// The receiver `a` of `a.f(b)`, passed as the parameter `at` of the method `f` asks, by
    /// the type typing found for it: a borrow of `a` in that parameter's mode, or `a` itself
    /// (spec 01, section 4).
    fn receiver(
        &mut self,
        f: &Operand,
        method: &Method,
        receiver: ExprId,
        at: usize,
    ) -> Result<Receiver, Unbuilt> {
        let name = method.1;
        let known = self.env.cx.get::<MemberType>(method).ok();
        let first = known.and_then(|k| k.ty.callable()).and_then(|s| s.params.get(at).copied());
        // A method whose type depends on its `$` parameters has a receiver written all the same.
        let written = || match f {
            Operand::Const(value) => {
                Callable::of(value.clone())?.modes(self.env())?.get(at).copied().flatten()
            }
            Operand::Place(_) => None,
        };
        match first.and_then(|t| t.borrowing()).map(|(mode, ..)| mode).or_else(written) {
            Some(mode) => {
                let origin = self.env.parsed.hir.exprs[receiver].origin;
                let place = self.reached(receiver, name)?;
                Ok(Receiver::Borrowed { mode, place, origin })
            }
            None => self.operand(receiver).map(Receiver::Value),
        }
    }
}
