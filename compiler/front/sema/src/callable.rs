//! What a call runs (spec 01, section 4): a function value, or a primitive, declared in `lang`
//! with its function type (decision 66). Both write their parameters the same way, `$` ones
//! included, so a call reads from where they are written which arguments are known at compile
//! time, which a call may leave out, and in which mode each is borrowed.

use vun_desugar::{Borrow, MARKERS};
use vun_hir::tree::{BinderId, Entry, ExprId, ExprKind, Param, PatKind};
use vun_query::Cx;

use crate::Primitive;
use crate::env::Env;
use crate::eval::Func;
use crate::ty::Value;

/// A function a call runs.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Callable {
    Fn(Func),
    Primitive(Primitive),
}

/// Where a callable's parameters are written: the environment of their file, and the function
/// literal or function type listing them.
pub struct Written<'a, 'db> {
    pub env: Env<'a, 'db>,
    pub expr: ExprId,
}

impl Callable {
    /// The callable a value is, if it is one.
    #[must_use]
    pub fn of(value: Value) -> Option<Self> {
        match value {
            Value::Fn(f) => Some(Self::Fn(f)),
            Value::Primitive(p) => Some(Self::Primitive(p)),
            _ => None,
        }
    }

    /// The value calling it.
    #[must_use]
    pub fn value(&self) -> Value {
        match self {
            Self::Fn(f) => Value::Fn(f.clone()),
            Self::Primitive(p) => Value::Primitive(*p),
        }
    }

    /// Where its parameters are written, as `env` finds it.
    #[must_use]
    pub fn written<'a, 'db>(&self, env: &Env<'a, 'db>) -> Option<Written<'a, 'db>> {
        match self {
            Self::Fn(f) => f.written(env.cx),
            Self::Primitive(p) => {
                let decl = env.primitive_decl(*p).ok()?;
                let env = Env::of(env.cx, &decl.file)?;
                let hir = &env.parsed.hir;
                let mut expr = hir.decls[decl.decl.id].ty?;
                // `unsafe (...) -> T`: the prefixes mark the signature (the table `markers`).
                while let ExprKind::Prefix { op, operand, .. } = &hir.exprs[expr].kind
                    && MARKERS.iter().any(|row| row.token == *op)
                {
                    expr = *operand;
                }
                Some(Written { env, expr })
            }
        }
    }

    /// Whether each parameter is a `$` parameter, known at compile time.
    #[must_use]
    pub fn comptime(&self, env: &Env<'_, '_>) -> Option<Vec<bool>> {
        self.written(env)?.comptime()
    }

    /// The borrow mode each parameter's annotation writes: see [`Written::modes`].
    #[must_use]
    pub fn modes(&self, env: &Env<'_, '_>) -> Option<Vec<Option<Borrow>>> {
        self.written(env)?.modes()
    }

    /// Whether each parameter receives its argument as code: see [`Func::quotes`]. No primitive
    /// quotes.
    #[must_use]
    pub fn quotes(&self, env: &Env<'_, '_>) -> Vec<bool> {
        match self {
            Self::Fn(f) => f.quotes(env.cx).unwrap_or_default(),
            Self::Primitive(_) => Vec::new(),
        }
    }
}

impl Func {
    /// Where this function's parameters are written: its literal, inside the call it was made in.
    #[must_use]
    pub fn written<'a, 'db>(&self, cx: &'a Cx<'db>) -> Option<Written<'a, 'db>> {
        Some(Written { env: Env::inside(cx, self, [])?, expr: self.literal.expr.id })
    }

    /// Whether each parameter is a `$` parameter; `None` when its file is not read.
    #[must_use]
    pub fn comptime(&self, cx: &Cx<'_>) -> Option<Vec<bool>> {
        self.written(cx)?.comptime()
    }

    /// The borrow mode each parameter's annotation writes: see [`Written::modes`].
    #[must_use]
    pub fn modes(&self, cx: &Cx<'_>) -> Option<Vec<Option<Borrow>>> {
        self.written(cx)?.modes()
    }
}

impl Written<'_, '_> {
    /// Each parameter's binder and annotation.
    #[must_use]
    pub fn params(&self) -> Option<Vec<(Option<BinderId>, Option<ExprId>)>> {
        let hir = &self.env.parsed.hir;
        let (ExprKind::Fn { params, .. } | ExprKind::FnType { params, .. }) = &hir.exprs[self.expr].kind
        else {
            return None;
        };
        params
            .iter()
            .map(|p| match p {
                Entry::One(Param { pat, ty, .. }) => {
                    let binder = pat.and_then(|p| match hir.pats[p].kind {
                        PatKind::Bind(b) => Some(b),
                        _ => None,
                    });
                    Some((binder, *ty))
                }
                Entry::Staged(_) => None,
            })
            .collect()
    }

    /// Whether each parameter is a `$` parameter, known at compile time.
    #[must_use]
    pub fn comptime(&self) -> Option<Vec<bool>> {
        let hir = &self.env.parsed.hir;
        Some(self.params()?.iter().map(|(b, _)| b.is_some_and(|b| hir.binders[b].comptime)).collect())
    }

    /// The borrow mode each parameter's annotation writes, whatever the `$` parameters: known
    /// before a call is instantiated. `None` for a parameter not written as a borrow.
    #[must_use]
    pub fn modes(&self) -> Option<Vec<Option<Borrow>>> {
        let hir = &self.env.parsed.hir;
        let mode = |(_, ty): &(Option<BinderId>, Option<ExprId>)| match hir.exprs[(*ty)?].kind {
            ExprKind::Borrow { mode, .. } => mode,
            _ => None,
        };
        Some(self.params()?.iter().map(mode).collect())
    }
}
