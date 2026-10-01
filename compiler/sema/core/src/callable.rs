//! What a call runs (spec 01, section 4): a function value, or a primitive, declared in `lang`
//! with its function type (decision 66). Both write their parameters the same way, `$` ones
//! included, so a call reads from where they are written which arguments are known at compile
//! time, which a call may leave out, and in which mode each is borrowed.

use vun_codec::{Decode, Encode};
use vun_desugar::{Borrow, MARKERS};
use vun_grammar::TokenKind;
use vun_hir::tree::{BinderId, Entry, ExprId, ExprKind, Param, PatKind};
use vun_primitives::Primitive;
use vun_primitives::meta::Meta;
use vun_query::Cx;
use vun_sema_ty::call::{Func, Instantiation};
use vun_sema_ty::trap::Unevaluated;
use vun_sema_ty::{Ty, TyKind, Value};

use crate::env::Env;
use crate::eval::Signed;

/// A function a call runs.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
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

impl Env<'_, '_> {
    /// The type of `value`, a function's included.
    ///
    /// # Errors
    /// Returns why its type is not known here.
    pub fn value_type(&self, value: &Value) -> Result<Ty, Unevaluated> {
        match value.ty() {
            Some(ty) => Ok(ty),
            None => Callable::of(value.clone()).ok_or(Unevaluated::Unsupported)?.ty(self),
        }
    }
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

    /// The parameter gathering the arguments after the others: see [`Written::pack`].
    #[must_use]
    pub fn pack(&self, env: &Env<'_, '_>) -> Option<usize> {
        self.written(env)?.pack()
    }

    /// Whether each parameter is a `$` parameter, known at compile time.
    #[must_use]
    pub fn comptime(&self, env: &Env<'_, '_>) -> Option<Vec<bool>> {
        self.written(env)?.comptime()
    }

    /// Its type: its signature's, or its own when it has `$` parameters.
    ///
    /// # Errors
    /// Returns why its signature is not known here, when it has none of its own.
    pub fn ty(&self, env: &Env<'_, '_>) -> Result<Ty, Unevaluated> {
        let signature = match self {
            Self::Fn(f) => env.cx.get::<Signed>(&Instantiation::new(f.clone(), Vec::new())),
            Self::Primitive(p) => env.primitive_signature(*p, &[]),
        };
        signature.map(|s| s.ty()).or_else(|why| self.generic(env).ok_or(why))
    }

    /// Its type when it has `$` parameters: itself, having no signature apart from the `$`
    /// arguments of a call ([`TyKind::Generic`]).
    #[must_use]
    pub fn generic(&self, env: &Env<'_, '_>) -> Option<Ty> {
        self.comptime(env)?.contains(&true).then(|| TyKind::Generic(Box::new(self.value())).intern())
    }

    /// Whether a call of it is known when compiling, whatever runs: one whose parameters are all
    /// `$` ones, so that nothing at run time changes its value. A primitive without parameters
    /// is such a call; a function needs a `$` parameter to be, or a call of it is a run-time one.
    #[must_use]
    pub fn folds(&self, env: &Env<'_, '_>) -> bool {
        self.comptime(env)
            .is_some_and(|c| (matches!(self, Self::Primitive(_)) || !c.is_empty()) && c.iter().all(|&c| c))
    }

    /// The borrow mode each parameter's annotation writes: see [`Written::modes`].
    #[must_use]
    pub fn modes(&self, env: &Env<'_, '_>) -> Option<Vec<Option<Borrow>>> {
        self.written(env)?.modes()
    }

    /// Whether each parameter receives its argument as code: see [`Written::quotes`].
    #[must_use]
    pub fn quotes(&self, env: &Env<'_, '_>) -> Option<Vec<bool>> {
        self.written(env)?.quotes()
    }

    /// Each parameter's type, where known before a call: see [`Written::types`].
    #[must_use]
    pub fn types(&self, env: &Env<'_, '_>) -> Option<Vec<Option<Ty>>> {
        self.written(env)?.types()
    }
}

/// A function literal's parameters, as written: which are `$` and how each is passed.
pub trait Parameters {
    /// Where this function's parameters are written: its literal, inside the call it was made in.
    #[must_use]
    fn written<'a, 'db>(&self, cx: &'a Cx<'db>) -> Option<Written<'a, 'db>>;

    /// Whether each parameter is a `$` parameter; `None` when its file is not read.
    #[must_use]
    fn comptime(&self, cx: &Cx<'_>) -> Option<Vec<bool>>;

    /// The borrow mode each parameter's annotation writes: see [`Written::modes`].
    #[must_use]
    fn modes(&self, cx: &Cx<'_>) -> Option<Vec<Option<Borrow>>>;
}

impl Parameters for Func {
    fn written<'a, 'db>(&self, cx: &'a Cx<'db>) -> Option<Written<'a, 'db>> {
        let env = Env::inside(cx, self, [])?;
        let expr = self.literal.expr.id;
        // A member of a witness that leaves its parameters' types out has them written by the
        // requirement it implements.
        if env.omits_param_types(expr)
            && let Some(Ok(asked)) = env.asked(expr)
        {
            return Some(asked);
        }
        Some(Written { env, expr })
    }

    fn comptime(&self, cx: &Cx<'_>) -> Option<Vec<bool>> {
        cx.get::<crate::fuel::Callee>(self).comptime
    }

    fn modes(&self, cx: &Cx<'_>) -> Option<Vec<Option<Borrow>>> {
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
            .map(|p| {
                let Param { pat, ty, .. } = p.written()?;
                let binder = pat.and_then(|p| match hir.pats[p].kind {
                    PatKind::Bind(b) | PatKind::Rest(Some(b)) => Some(b),
                    _ => None,
                });
                Some((binder, *ty))
            })
            .collect()
    }

    /// The expression of the result type, when the function writes one.
    #[must_use]
    pub fn returns(&self) -> Option<ExprId> {
        let (ExprKind::Fn { returns, .. } | ExprKind::FnType { returns, .. }) =
            &self.env.parsed.hir.exprs[self.expr].kind
        else {
            return None;
        };
        *returns
    }

    /// The parameter gathering the arguments after the others, when it has one: the last, a
    /// rest pattern `..args: P` in a function literal, `..P` in a function type.
    #[must_use]
    pub fn pack(&self) -> Option<usize> {
        let hir = &self.env.parsed.hir;
        let (ExprKind::Fn { params, .. } | ExprKind::FnType { params, .. }) = &hir.exprs[self.expr].kind
        else {
            return None;
        };
        let last = params.len().checked_sub(1)?;
        let Some(Entry::One(Param { pat, ty, .. })) = params.iter().last() else { return None };
        let packed = match (pat, ty) {
            (Some(p), _) => matches!(hir.pats[*p].kind, PatKind::Rest(_)),
            (None, Some(t)) => matches!(hir.exprs[*t].kind, ExprKind::Prefix { op: TokenKind::DotDot, .. }),
            (None, None) => false,
        };
        packed.then_some(last)
    }

    /// Whether each parameter is a `$` parameter, known at compile time.
    #[must_use]
    pub fn comptime(&self) -> Option<Vec<bool>> {
        let hir = &self.env.parsed.hir;
        Some(self.params()?.iter().map(|(b, _)| b.is_some_and(|b| hir.binders[b].comptime)).collect())
    }

    /// Each parameter's type, where its annotation names no `$` parameter: known before a call
    /// is instantiated.
    #[must_use]
    pub fn types(&self) -> Option<Vec<Option<Ty>>> {
        let ty = |(_, ty): &(Option<BinderId>, Option<ExprId>)| self.env.eval((*ty)?).ok()?.as_type();
        Some(self.params()?.iter().map(ty).collect())
    }

    /// Whether each parameter receives its argument as code: those of type `lang.meta.Code(T)`,
    /// whatever `T` (spec 03, section 4).
    #[must_use]
    pub fn quotes(&self) -> Option<Vec<bool>> {
        let hir = &self.env.parsed.hir;
        let quotes = |(_, ty): &(Option<BinderId>, Option<ExprId>)| {
            let Some(ExprKind::Call { callee, .. }) = ty.map(|t| &hir.exprs[t].kind) else { return false };
            matches!(self.env.eval(*callee), Ok(Value::Primitive(Primitive::Meta(Meta::Code))))
        };
        Some(self.params()?.iter().map(quotes).collect())
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

/// An instantiation as generic code or not.
pub trait Generic {
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
