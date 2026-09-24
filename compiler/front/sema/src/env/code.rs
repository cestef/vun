//! Code values at compile time (spec 03, section 4): a quote or a `Code(T)` argument is code
//! written here, and splicing it with `$` evaluates it where it was written.

use std::sync::Arc;

use vun_hir::tree::{Entry, ExprId, ExprKind, Param};
use vun_query::Cx;

use super::{Env, Unevaluated};
use crate::Primitive;
use crate::eval::{Code, Func, Staging};
use crate::item::{ExprRef, Node};
use crate::primitives::meta::Meta;
use crate::ty::Value;

impl<'a, 'db> Env<'a, 'db> {
    /// The code `expr`, written here: it keeps the call evaluated in and every binder bound, so
    /// its names mean what they mean here.
    #[must_use]
    pub fn code(&self, expr: ExprId) -> Code {
        let hir = &self.parsed.hir;
        let mut bound: Vec<_> = self
            .bindings
            .iter()
            .map(|(&b, value)| Staging {
                binder: Node { id: b, origin: hir.binders[b].origin },
                value: value.clone(),
            })
            .collect();
        bound.sort_by_key(|s| s.binder);
        Code {
            expr: ExprRef::new(self.file.clone(), hir, expr),
            within: self.instance.clone().map(Arc::new),
            bound: Arc::from(bound),
        }
    }

    /// The environment `code` was written in.
    #[must_use]
    pub fn quoted(cx: &'a Cx<'db>, code: &Code) -> Option<Self> {
        let env = match &code.within {
            Some(call) => Self::within(cx, call)?,
            None => Self::of(cx, &code.expr.file)?,
        };
        Some(env.staging(&code.bound))
    }

    /// `$value`: code is evaluated where it was written; any other value is itself.
    ///
    /// # Errors
    /// Returns why the code has no value at compile time.
    pub fn splice(&self, value: Value) -> Result<Value, Unevaluated> {
        let Value::Code(code) = value else { return Ok(value) };
        let env = Self::quoted(self.cx, &code).ok_or(Unevaluated::Absent)?;
        match &env.parsed.hir.exprs[code.expr.expr.id].kind {
            // A quote's value is that of its body.
            ExprKind::Quote(block) => env.block_value(*block),
            _ => env.eval(code.expr.expr.id),
        }
    }
}

impl Func {
    /// Whether each parameter of this function receives its argument as code: those of type
    /// `lang.meta.Code(T)`, whatever `T` (spec 03, section 4). `None` when its file is not read.
    #[must_use]
    pub fn quotes(&self, cx: &Cx<'_>) -> Option<Vec<bool>> {
        let env = Env::inside(cx, self, [])?;
        let hir = &env.parsed.hir;
        let ExprKind::Fn { params, .. } = &hir.exprs[self.literal.expr.id].kind else { return None };
        let quotes = params.iter().map(|p| {
            let Entry::One(Param { ty: Some(ty), .. }) = p else { return false };
            let ExprKind::Call { callee, .. } = &hir.exprs[*ty].kind else { return false };
            matches!(env.eval(*callee), Ok(Value::Primitive(Primitive::Meta(Meta::Code))))
        });
        Some(quotes.collect())
    }
}
