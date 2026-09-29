//! The literals a declaration's value comes from, read where they are written: whether what a
//! struct or a function literal gives is a namespace or a type (spec 10, section 5).

use vun_hir::tree::{Entry, ExprId, ExprKind, Member};
use vun_query::Cx;
use vun_sema_ty::TyKind;
use vun_sema_ty::item::ExprRef;

use crate::callable::{Callable, Written};
use crate::env::Env;

/// A literal, with the file it is written in: a value's, or the annotation of a primitive's
/// declaration.
pub(super) struct Literal<'a, 'db> {
    env: Env<'a, 'db>,
    at: ExprRef,
}

impl<'a, 'db> From<Written<'a, 'db>> for Literal<'a, 'db> {
    fn from(written: Written<'a, 'db>) -> Self {
        let at = ExprRef::new(written.env.file.clone(), &written.env.parsed.hir, written.expr);
        Self { env: written.env, at }
    }
}

impl<'a, 'db> Literal<'a, 'db> {
    pub(super) fn of(cx: &'a Cx<'db>, at: &ExprRef) -> Option<Self> {
        Some(Self { env: Env::of(cx, &at.file)?, at: at.clone() })
    }

    fn kind(&self) -> &ExprKind {
        &self.env.parsed.hir.exprs[self.at.expr.id].kind
    }

    /// Whether this struct literal is a namespace: declarations or spreads of them, no field, and
    /// no method receiving the struct itself, which makes it a type with no data.
    pub(super) fn namespace(&self) -> bool {
        let hir = &self.env.parsed.hir;
        let ExprKind::Struct(ns) = *self.kind() else { return false };
        let (mut decls, mut fields) = (false, false);
        for member in hir.namespaces[ns].members.iter().filter_map(Entry::written) {
            match member {
                Member::Decl(d) if self.receives(hir.decls[*d].value) => return false,
                Member::Decl(_) | Member::Spread(_) => decls = true,
                Member::Field(_) => fields = true,
            }
        }
        decls && !fields
    }

    /// Whether `value`, the value of one of this struct's declarations, is a function whose first
    /// parameter is the struct, by value or through a borrow or a pointer: a function literal, or
    /// a function or primitive it names (`at := lang.arch.at`).
    fn receives(&self, value: Option<ExprId>) -> bool {
        let Some(callable) = value.and_then(|v| Callable::of(self.env.eval(v).ok()?)) else { return false };
        let Some(Some(ty)) = callable.types(&self.env).and_then(|types| types.into_iter().next()) else {
            return false;
        };
        let ty = ty.pointee().unwrap_or(ty);
        matches!(&*ty, TyKind::Nominal(n) if n.literal == self.at)
    }

    /// Whether what this literal gives is a type or an interface other than a namespace: a
    /// function literal or type whose result is annotated `type` or `lang.Interface`, a function
    /// literal's body not ending in a namespace; a primitive's annotation that is not a
    /// function type, when it is `type` or `lang.Interface` itself.
    pub(super) fn gives_type(&self) -> bool {
        let hir = &self.env.parsed.hir;
        let types = |annotation: ExprId| self.env.ty(annotation).is_ok_and(|t| t.holds_types());
        match self.kind() {
            ExprKind::Fn { returns: Some(returns), body, .. } => {
                let tail = hir.blocks[*body].tail.map(|t| ExprRef::new(self.at.file.clone(), hir, t));
                types(*returns)
                    && !tail.and_then(|t| Self::of(self.env.cx, &t)).is_some_and(|t| t.namespace())
            }
            ExprKind::FnType { returns: Some(returns), .. } => types(*returns),
            ExprKind::Fn { .. } | ExprKind::FnType { .. } => false,
            _ => types(self.at.expr.id),
        }
    }
}
