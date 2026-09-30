//! The parameters of a function, as reflection gives them: each as a part of the tuple a call
//! passes, by its name, position, type and the values of the attributes written on it.

use vun_hir::tree::{ExprKind, PatKind};
use vun_intern::Symbol;
use vun_items::lang;
use vun_sema_ty::trap::Unevaluated;
use vun_sema_ty::{Ty, Value};

use super::{PART, Part};
use crate::env::Env;

impl Env<'_, '_> {
    /// `lang.meta.params(F, f)`: a descriptor of each parameter of the function `f`, of type `F`,
    /// named as its literal names it, typed as `F` types it.
    pub(crate) fn params(&self, ty: Ty, f: &Value) -> Result<Value, Unevaluated> {
        let signature = ty.signature().ok_or(Unevaluated::Unsupported)?;
        let Value::Fn(f) = f else { return Err(Unevaluated::Unsupported) };
        let env = Env::defining(self.cx, &f.literal, f.within.as_deref()).ok_or(Unevaluated::Absent)?;
        let hir = &env.parsed.hir;
        let ExprKind::Fn { params, .. } = &hir.exprs[f.literal.expr.id].kind else {
            return Err(Unevaluated::Unsupported);
        };
        let written = params.iter().map(|p| p.written().ok_or(Unevaluated::Unsupported));
        let parts = written.zip(signature.params).enumerate().map(|(index, (param, ty))| {
            let param = param?;
            let name = match param.pat.map(|p| &hir.pats[p].kind) {
                Some(PatKind::Bind(b) | PatKind::Rest(Some(b))) => hir.binders[*b].name,
                _ => Symbol::intern(&index.to_string()),
            };
            Ok(Part { name, index, ty: Ok(ty), attrs: env.values(&param.attrs) })
        });
        self.describe(lang::meta::Field, &parts.collect::<Result<Vec<_>, _>>()?, PART)
    }
}
