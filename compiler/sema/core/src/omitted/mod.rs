//! `$` arguments left out of a call (decision 99): a `$` parameter named in the type of a later
//! parameter may be omitted, all such parameters or none, and is inferred by matching the types
//! of the arguments against the types the parameters are declared with.

mod plan;

use std::collections::{BTreeMap, BTreeSet};

pub use plan::Plan;
use vun_desugar::{Form, TYPE_PREFIXES};
use vun_hir::resolve::Res;
use vun_hir::tree::{BinderId, Entry, ExprId, ExprKind, Hir, Param};
use vun_hir::visit::{self, Visitor};
use vun_intern::Symbol;
use vun_sema_ty::{Ty, TyKind, Value};

use crate::callable::Callable;
use crate::env::Env;

/// Whether a type annotation names a binder.
struct Named<'e, 'a, 'db> {
    env: &'e Env<'a, 'db>,
    binder: BinderId,
    found: bool,
}

impl Named<'_, '_, '_> {
    fn names(env: &Env<'_, '_>, ty: ExprId, binder: BinderId) -> bool {
        let mut named = Named { env, binder, found: false };
        named.expr(&env.parsed.hir, ty);
        named.found
    }
}

impl Visitor for Named<'_, '_, '_> {
    fn expr(&mut self, hir: &Hir, expr: ExprId) {
        if self.env.parsed.resolution.names[expr] == Some(Res::Binder(self.binder)) {
            self.found = true;
        }
        visit::walk_expr(self, hir, expr);
    }
}

/// Values for `$` parameters, found by matching the types they are named in against the types
/// of the arguments.
pub struct Unifier<'e, 'a, 'db> {
    env: &'e Env<'a, 'db>,
    wanted: BTreeSet<BinderId>,
    bound: BTreeMap<BinderId, Value>,
}

impl Unifier<'_, '_, '_> {
    /// Infers the omitted parameters of `plan` for a call of `f` whose written arguments have
    /// the types `args`; each is `None` when nothing determines it.
    ///
    /// `expected` is the type the call is expected to have, which its callee's result type is
    /// matched against as one more source: a `$` parameter the result names is at least as wide
    /// as what the context takes (`fold(0, f)` where a `u32` is expected is `u32`, not `0`'s
    /// exact type).
    #[must_use]
    pub fn infer(
        env: &Env<'_, '_>,
        f: &Callable,
        plan: &Plan,
        args: &[Option<Ty>],
        expected: Option<Ty>,
    ) -> Option<Vec<Option<Value>>> {
        let written = f.written(env)?;
        let params = written.params()?;
        let wanted = plan.omitted.iter().filter_map(|&i| params[i].0).collect();
        let mut unifier = Unifier { env: &written.env, wanted, bound: BTreeMap::new() };
        for (slot, arg) in plan.grouped(args) {
            if let (Some(ty), Some(annotation)) = (arg, params[slot].1) {
                unifier.unify(annotation, &Value::Type(ty));
            }
        }
        if let Some(expected) = expected
            && let Some(returns) = written.returns()
        {
            unifier.unify(returns, &Value::Type(expected));
        }
        // A parameter found to be a type may fix others through its constraint: `U` in
        // `$F: Call((T), U)` from the signature of the function or closure `F` is.
        loop {
            let found = unifier.bound.len();
            for &i in &plan.omitted {
                let (Some(binder), Some(annotation)) = params[i] else { continue };
                if let Some(Value::Type(ty)) = unifier.bound.get(&binder).cloned() {
                    unifier.constraint(annotation, ty);
                }
            }
            if unifier.bound.len() == found {
                break;
            }
        }
        Some(plan.omitted.iter().map(|&i| params[i].0.and_then(|b| unifier.bound.get(&b).cloned())).collect())
    }

    /// The names of the binders of `plan`'s omitted parameters.
    #[must_use]
    pub fn names(env: &Env<'_, '_>, f: &Callable, plan: &Plan) -> Vec<Symbol> {
        let Some(written) = f.written(env) else { return Vec::new() };
        let Some(params) = written.params() else { return Vec::new() };
        let hir = &written.env.parsed.hir;
        plan.omitted.iter().filter_map(|&i| params[i].0).map(|b| hir.binders[b].name).collect()
    }

    /// Matches the annotation `pattern` against `value`, binding the wanted binders it names.
    fn unify(&mut self, pattern: ExprId, value: &Value) {
        let env = self.env;
        let hir = &env.parsed.hir;
        let ty = value.as_type();
        match &hir.exprs[pattern].kind {
            ExprKind::Name(_) => {
                if let Some(Res::Binder(b)) = env.parsed.resolution.names[pattern]
                    && self.wanted.contains(&b)
                {
                    self.bind(b, value);
                }
            }
            ExprKind::Borrow { mode, operand } => {
                if let Some((m, _, pointee)) = ty.and_then(|t| t.borrowing())
                    && Some(m) == *mode
                {
                    self.unify(*operand, &Value::Type(pointee));
                }
            }
            ExprKind::Prefix { op, operand, .. } => {
                if let Some(row) = TYPE_PREFIXES.iter().find(|row| row.token == *op)
                    && let Ok(constructor) = env.item(row.item)
                {
                    self.parts(&constructor, ty, &[*operand]);
                }
            }
            ExprKind::Call { callee, args } => {
                let args: Option<Vec<ExprId>> = args
                    .iter()
                    .map(|a| match a {
                        Entry::One(a) if a.name.is_none() => Some(a.value),
                        _ => None,
                    })
                    .collect();
                if let (Ok(constructor), Some(args)) = (env.eval(*callee), args) {
                    self.parts(&constructor, ty, &args);
                }
            }
            ExprKind::ArrayType { len, elem } => {
                if let Ok(constructor) = env.form(Form::Array) {
                    self.parts(&constructor, ty, &[*len, *elem]);
                }
            }
            // A slice type names its items in an array's too, which has the impls of its slice.
            ExprKind::Array(items) => {
                if let ([Entry::One(item)], Some(found)) = (items.as_slice(), ty.and_then(|t| t.items())) {
                    self.unify(*item, &Value::Type(found));
                }
            }
            ExprKind::Tuple(items) => {
                if let Some(TyKind::Tuple(parts)) = ty.as_deref() {
                    for (item, part) in items.iter().zip(parts) {
                        if let Entry::One(item) = item {
                            self.unify(*item, &Value::Type(*part));
                        }
                    }
                }
            }
            ExprKind::FnType { params, returns } => {
                if let Some(signature) = ty.and_then(|t| t.callable()) {
                    for (param, found) in params.iter().zip(&signature.params) {
                        if let Entry::One(Param { ty: Some(annotation), .. }) = param {
                            self.unify(*annotation, &Value::Type(*found));
                        }
                    }
                    if let Some(returns) = returns {
                        self.unify(*returns, &Value::Type(signature.result));
                    }
                }
            }
            _ => {}
        }
    }

    /// Matches the constraint `annotation`, an interface applied to arguments, against the
    /// arguments of that interface `ty` implements.
    fn constraint(&mut self, annotation: ExprId, ty: Ty) {
        let env = self.env;
        let ExprKind::Call { callee, args } = &env.parsed.hir.exprs[annotation].kind else { return };
        let Ok(constructor) = env.eval(*callee) else { return };
        let Some(made) = env.implemented(ty, &constructor) else { return };
        for (arg, part) in args.iter().zip(&made) {
            if let Entry::One(arg) = arg {
                self.unify(arg.value, part);
            }
        }
    }

    /// Matches the arguments written for `constructor` against those `ty` was made with.
    fn parts(&mut self, constructor: &Value, ty: Option<Ty>, args: &[ExprId]) {
        let made = ty.and_then(|ty| match constructor {
            Value::Primitive(_) | Value::Fn(_) => ty.made_by(constructor),
            _ => None,
        });
        for (arg, part) in args.iter().zip(made.iter().flatten()) {
            self.unify(*arg, part);
        }
    }

    /// Binds `binder` to `value`; types found twice join, as the branches of an `if` do.
    fn bind(&mut self, binder: BinderId, value: &Value) {
        let joined = match (self.bound.get(&binder), value.as_type()) {
            (None, _) => Some(value.clone()),
            (Some(Value::Type(old)), Some(new)) => old.join(&new).map(Value::Type),
            (Some(_), _) => None,
        };
        if let Some(joined) = joined {
            self.bound.insert(binder, joined);
        }
    }
}
