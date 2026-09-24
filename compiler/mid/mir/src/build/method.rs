//! Method calls: `a.f(b)` is `T.f(a, b)`, with the method the typer found through `T`'s
//! namespace or impls (spec 01, section 4).

use overt_hir::tree::{ExprId, ExprKind};
use overt_sema::member::MemberValue;

use super::{Builder, Unbuilt};
use crate::body::Operand;

impl Builder<'_, '_, '_> {
    /// The method `callee` names through a value, and the value; `None` when `callee` names no
    /// method.
    pub(super) fn method(&self, callee: ExprId) -> Result<Option<(Operand, ExprId)>, Unbuilt> {
        let e = &self.env.parsed.hir.exprs[callee];
        let ExprKind::Member { object, .. } = e.kind else { return Ok(None) };
        let unbuilt = Unbuilt { at: e.origin };
        let typed = self.typed().ok_or(unbuilt)?;
        let Some(method) = typed.methods.get(&callee) else { return Ok(None) };
        let f = self.env.cx.get::<MemberValue>(method).map_err(|_| unbuilt)?;
        Ok(Some((Operand::Const(f), object)))
    }
}
