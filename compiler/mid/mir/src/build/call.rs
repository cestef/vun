//! Calls: `a.f(b)` is `T.f(a, b)`, with the method the typer found through `T`'s namespace or
//! impls (spec 01, section 4), and an argument for a `Code(T)` parameter is passed as code
//! (spec 03, section 4).

use overt_hir::tree::{Arg, Entry, ExprId, ExprKind};
use overt_origin::OriginId;
use overt_sema::member::MemberValue;
use overt_sema::ty::Value;

use super::{Builder, Unbuilt};
use crate::body::{Operand, Rvalue};

impl Builder<'_, '_, '_> {
    pub(super) fn call(
        &mut self,
        callee: ExprId,
        args: &[Entry<Arg>],
        origin: OriginId,
    ) -> Result<Operand, Unbuilt> {
        let unbuilt = Unbuilt { at: origin };
        // `a.f(b)` is `T.f(a, b)`, `f` the method the typer found (spec 01, section 4).
        let (callee, mut values) = match self.method(callee)? {
            Some((f, receiver)) => (f, vec![self.operand(receiver)?]),
            None => (self.operand(callee)?, Vec::new()),
        };
        // A `Code(T)` parameter receives its argument as code.
        let quotes = match &callee {
            Operand::Const(Value::Fn(f)) => f.quotes(self.env.cx).unwrap_or_default(),
            _ => Vec::new(),
        };
        for arg in args {
            match arg {
                Entry::One(Arg { name: None, value }) if quotes.get(values.len()) == Some(&true) => {
                    let code = self.quote(*value, origin);
                    values.push(code);
                }
                Entry::One(Arg { name: None, value }) => values.push(self.operand(*value)?),
                _ => return Err(unbuilt),
            }
        }
        Ok(self.temp(Rvalue::Call { callee, args: values }, origin))
    }

    /// The method `callee` names through a value, and the value; `None` when `callee` names no
    /// method.
    fn method(&self, callee: ExprId) -> Result<Option<(Operand, ExprId)>, Unbuilt> {
        let e = &self.env.parsed.hir.exprs[callee];
        let ExprKind::Member { object, .. } = e.kind else { return Ok(None) };
        let unbuilt = Unbuilt { at: e.origin };
        let typed = self.typed().ok_or(unbuilt)?;
        let Some(method) = typed.methods.get(&callee) else { return Ok(None) };
        let f = self.env.cx.get::<MemberValue>(method).map_err(|_| unbuilt)?;
        Ok(Some((Operand::Const(f), object)))
    }
}
