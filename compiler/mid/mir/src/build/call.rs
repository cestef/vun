//! Calls: `a.f(b)` is `T.f(a, b)`, with the method the typer found through `T`'s namespace or
//! impls (spec 01, section 4), and an argument for a `Code(T)` parameter is passed as code
//! (spec 03, section 4).

use vun_desugar::ITERATION;
use vun_hir::tree::{Arg, Entry, ExprId, ExprKind, Name};
use vun_intern::Symbol;
use vun_origin::OriginId;
use vun_sema::member::{MemberType, MemberValue};
use vun_sema::ty::{Ty, TyKind, Value};

use super::{Builder, Unbuilt};
use crate::body::{Aggregate, Operand, Rvalue, TerminatorKind};
use crate::place::Place;

/// A method, by the type it is found through and its name: the key of `MemberType`.
type Method = (Ty, Symbol);

/// How a method's receiver is passed: borrowed in a mode, from a place computed before the
/// arguments, or as a value.
enum Receiver {
    Borrowed { mode: Option<Symbol>, place: Place, origin: OriginId },
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
        let fill = |values: &mut Vec<Operand>| {
            while let Some((_, value)) = inferred.iter().find(|(slot, _)| *slot == values.len()) {
                values.push(Operand::Const(value.clone()));
            }
        };
        let mut values: Vec<Operand> = Vec::new();
        fill(&mut values);
        // `a.f(b)` is `T.f(a, b)`, `f` the method the typer found (spec 01, section 4): the
        // receiver is the first written argument, after the `$` arguments inferred before it.
        let at = values.len();
        let (callee, receiver) = match self.method(callee)? {
            Some((f, receiver, method)) => {
                let receiver = self.receiver(&f, &method, receiver, at)?;
                (f, Some(receiver))
            }
            None => (self.operand(callee)?, None),
        };
        // The receiver's slot, filled once the arguments are evaluated.
        if receiver.is_some() {
            values.push(Self::unit());
        }
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
        // A `Code(T)` parameter receives its argument as code.
        let quotes = match &callee {
            Operand::Const(Value::Fn(f)) => f.quotes(self.env.cx).unwrap_or_default(),
            _ => Vec::new(),
        };
        fill(&mut values);
        for arg in args {
            match arg {
                Entry::One(Arg { name: None, value }) if quotes.get(values.len()) == Some(&true) => {
                    let code = self.quote(*value, origin);
                    values.push(code);
                }
                Entry::One(Arg { name: None, value }) => values.push(self.operand(*value)?),
                _ => return Err(unbuilt),
            }
            fill(&mut values);
        }
        // A borrowed receiver is borrowed after the arguments are evaluated, its place computed
        // before them: `v.push(v.len())` reads `v` before `&mut v` is taken (two-phase).
        match receiver {
            Some(Receiver::Borrowed { mode, place, origin }) => {
                values[at] = self.temp(Rvalue::Ref { mode, place }, origin);
            }
            Some(Receiver::Value(operand)) => values[at] = operand,
            None => {}
        }
        let result = self.temp(Rvalue::Call { callee, args: values }, origin);
        // A call of an empty type, such as `never`, does not come back.
        if self.type_of(call).is_some_and(|t| t.is_empty()) {
            self.terminate(TerminatorKind::Unreachable, origin);
            self.current = self.fresh(origin);
        }
        Ok(result)
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
        let Some(&(_, _, _, resume, end, ..)) = ITERATION.first() else { return Ok(None) };
        if !framed || !args.is_empty() {
            return Ok(None);
        }
        let rvalue = if name.as_str() == resume {
            let step = self.type_of(call).ok_or(Unbuilt { at: origin })?;
            Rvalue::Resume { frame: self.through_borrows(object)?, step }
        } else if name.as_str() == end {
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
            Operand::Const(Value::Fn(f)) => f.modes(self.env.cx)?.get(at).copied().flatten(),
            _ => None,
        };
        match first.and_then(|t| t.borrowing()).map(|(mode, ..)| mode).or_else(written) {
            Some(mode) => {
                let origin = self.env.parsed.hir.exprs[receiver].origin;
                let place = self.reached(receiver, name)?;
                let mode = (!mode.is_empty()).then(|| Symbol::intern(mode));
                Ok(Receiver::Borrowed { mode, place, origin })
            }
            None => self.operand(receiver).map(Receiver::Value),
        }
    }
}
