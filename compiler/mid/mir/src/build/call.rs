//! Calls: `a.f(b)` is `T.f(a, b)`, with the method the typer found through `T`'s namespace or
//! impls (spec 01, section 4), and an argument for a `Code(T)` parameter is passed as code
//! (spec 03, section 4).

use overt_hir::tree::{Arg, Entry, ExprId, ExprKind};
use overt_intern::Symbol;
use overt_origin::OriginId;
use overt_sema::env::Env;
use overt_sema::member::MemberValue;
use overt_sema::ty::Value;

use super::{Builder, Unbuilt};
use crate::body::{Aggregate, Operand, Rvalue};

impl Builder<'_, '_, '_> {
    pub(super) fn call(
        &mut self,
        call: ExprId,
        callee: ExprId,
        args: &[Entry<Arg>],
        origin: OriginId,
    ) -> Result<Operand, Unbuilt> {
        let unbuilt = Unbuilt { at: origin };
        // `a.f(b)` is `T.f(a, b)`, `f` the method the typer found (spec 01, section 4).
        let (callee, mut values) = match self.method(callee)? {
            Some((f, receiver, name)) => {
                let receiver = self.receiver(&f, receiver, name)?;
                (f, vec![receiver])
            }
            None => (self.operand(callee)?, Vec::new()),
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
        // A `Code(T)` parameter receives its argument as code.
        let quotes = match &callee {
            Operand::Const(Value::Fn(f)) => f.quotes(self.env.cx).unwrap_or_default(),
            _ => Vec::new(),
        };
        // The `$` arguments the call leaves out, as the typer inferred them.
        let inferred = self.typed().and_then(|t| t.inferred.get(&call).cloned()).unwrap_or_default();
        let fill = |values: &mut Vec<Operand>| {
            while let Some((_, value)) = inferred.iter().find(|(slot, _)| *slot == values.len()) {
                values.push(Operand::Const(value.clone()));
            }
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
        Ok(self.temp(Rvalue::Call { callee, args: values }, origin))
    }

    /// The method `callee` names through a value, and the value; `None` when `callee` names no
    /// method.
    fn method(&self, callee: ExprId) -> Result<Option<(Operand, ExprId, Symbol)>, Unbuilt> {
        let e = &self.env.parsed.hir.exprs[callee];
        let ExprKind::Member { object, .. } = e.kind else { return Ok(None) };
        let unbuilt = Unbuilt { at: e.origin };
        let typed = self.typed().ok_or(unbuilt)?;
        let Some(method) = typed.methods.get(&callee) else { return Ok(None) };
        let f = self.env.cx.get::<MemberValue>(method).map_err(|_| unbuilt)?;
        Ok(Some((Operand::Const(f), object, method.1)))
    }

    /// The receiver `a` of `a.f(b)`, passed as `f`'s first parameter asks: a borrow of `a` in
    /// that parameter's mode, or `a` itself (spec 01, section 4).
    fn receiver(&mut self, f: &Operand, receiver: ExprId, name: Symbol) -> Result<Operand, Unbuilt> {
        let first = match f {
            Operand::Const(Value::Fn(func)) => Env::inside(self.env.cx, func, [])
                .and_then(|env| env.literal_signature(func.literal.expr.id, &[]).ok())
                .and_then(|s| s.params.first().copied()),
            _ => None,
        };
        match first.and_then(|t| t.borrowing()) {
            Some((mode, ..)) => {
                let origin = self.env.parsed.hir.exprs[receiver].origin;
                let place = self.reached(receiver, name)?;
                let mode = (!mode.is_empty()).then(|| Symbol::intern(mode));
                Ok(self.temp(Rvalue::Ref { mode, place }, origin))
            }
            None => self.operand(receiver),
        }
    }
}
