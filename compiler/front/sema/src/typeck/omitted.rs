//! `$` arguments left out of a call (decision 99): a `$` parameter named in the type of a later
//! parameter may be omitted, all such parameters or none, and is inferred by matching the types
//! of the arguments against the types the parameters are declared with.

use std::collections::{BTreeMap, BTreeSet};

use overt_grammar::{Form, TYPE_PREFIXES, TokenKind};
use overt_hir::resolve::Res;

use overt_hir::tree::{BinderId, Entry, ExprId, ExprKind, Hir, Param, PatKind};
use overt_hir::visit::{self, Visitor};
use overt_intern::Symbol;

use crate::env::Env;
use crate::eval::Func;
use crate::ty::{Nominal, Ty, TyKind, Value};

/// Which parameter each written argument of a call is for.
pub(super) struct Plan {
    /// The parameter of each written argument, the receiver's first.
    pub(super) slots: Vec<usize>,
    /// The `$` parameters left out, to infer.
    pub(super) omitted: Vec<usize>,
}

impl Plan {
    /// How `written` arguments, the receiver included, go to the parameters of `f`: one each,
    /// or one each but for every `$` parameter a later parameter's type names.
    pub(super) fn of(env: &Env<'_, '_>, f: &Func, written: usize) -> Option<Self> {
        let inferable = Self::inferable(env, f)?;
        if written == inferable.len() {
            return Some(Self { slots: (0..written).collect(), omitted: Vec::new() });
        }
        let omitted: Vec<usize> = (0..inferable.len()).filter(|&i| inferable[i]).collect();
        let slots: Vec<usize> = (0..inferable.len()).filter(|&i| !inferable[i]).collect();
        (!omitted.is_empty() && slots.len() == written).then_some(Self { slots, omitted })
    }

    /// The argument counts a call of `f` may pass, the receiver included: every parameter, or
    /// every one but the inferable `$` parameters when there are some.
    pub(super) fn counts(env: &Env<'_, '_>, f: &Func) -> Vec<usize> {
        let Some(inferable) = Self::inferable(env, f) else { return Vec::new() };
        let without = inferable.iter().filter(|&&i| !i).count();
        if without == inferable.len() { vec![inferable.len()] } else { vec![inferable.len(), without] }
    }

    /// Whether each parameter of `f` is a `$` parameter named in a later parameter's type.
    fn inferable(env: &Env<'_, '_>, f: &Func) -> Option<Vec<bool>> {
        let inside = Env::inside(env.cx, f, [])?;
        let hir = &inside.parsed.hir;
        let params = Params::of(hir, f.literal.expr.id)?;
        Some(
            params
                .iter()
                .enumerate()
                .map(|(i, (binder, _))| {
                    binder.filter(|&b| hir.binders[b].comptime).is_some_and(|b| {
                        params[i + 1..].iter().any(|(_, ty)| ty.is_some_and(|t| Named::names(&inside, t, b)))
                    })
                })
                .collect(),
        )
    }
}

/// The binder and the type annotation of each parameter of a function literal.
struct Params;

impl Params {
    fn of(hir: &Hir, literal: ExprId) -> Option<Vec<(Option<BinderId>, Option<ExprId>)>> {
        let ExprKind::Fn { params, .. } = &hir.exprs[literal].kind else { return None };
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
}

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
pub(super) struct Unifier<'e, 'a, 'db> {
    env: &'e Env<'a, 'db>,
    wanted: BTreeSet<BinderId>,
    bound: BTreeMap<BinderId, Value>,
}

impl Unifier<'_, '_, '_> {
    /// Infers the omitted parameters of `plan` for a call of `f` whose written arguments have
    /// the types `args`; each is `None` when nothing determines it.
    pub(super) fn infer(
        env: &Env<'_, '_>,
        f: &Func,
        plan: &Plan,
        args: &[Option<Ty>],
    ) -> Option<Vec<Option<Value>>> {
        let inside = Env::inside(env.cx, f, [])?;
        let params = Params::of(&inside.parsed.hir, f.literal.expr.id)?;
        let wanted = plan.omitted.iter().filter_map(|&i| params[i].0).collect();
        let mut unifier = Unifier { env: &inside, wanted, bound: BTreeMap::new() };
        for (&slot, arg) in plan.slots.iter().zip(args) {
            if let (Some(ty), Some(annotation)) = (arg, params[slot].1) {
                unifier.unify(annotation, &Value::Type(*ty));
            }
        }
        Some(plan.omitted.iter().map(|&i| params[i].0.and_then(|b| unifier.bound.get(&b).cloned())).collect())
    }

    /// The names of the binders of `plan`'s omitted parameters.
    pub(super) fn names(env: &Env<'_, '_>, f: &Func, plan: &Plan) -> Vec<Symbol> {
        let Some(inside) = Env::inside(env.cx, f, []) else { return Vec::new() };
        let hir = &inside.parsed.hir;
        let Some(params) = Params::of(hir, f.literal.expr.id) else { return Vec::new() };
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
            ExprKind::Prefix { op: TokenKind::Amp, mode, operand } => {
                let word = mode.map_or("", Symbol::as_str);
                if let Some((m, _, pointee)) = ty.and_then(|t| t.borrowing())
                    && m == word
                {
                    self.unify(*operand, &Value::Type(pointee));
                }
            }
            ExprKind::Prefix { op, operand, .. } => {
                if let Some((_, item)) = TYPE_PREFIXES.iter().find(|(t, _)| t == op)
                    && let Ok(constructor) = env.item(item)
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
            ExprKind::Array(items) => {
                if let ([Entry::One(item)], Ok(constructor)) = (items.as_slice(), env.form(Form::Slice)) {
                    self.parts(&constructor, ty, &[*item]);
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

    /// Matches the arguments written for `constructor` against those `ty` was made with.
    fn parts(&mut self, constructor: &Value, ty: Option<Ty>, args: &[ExprId]) {
        let made = ty.and_then(|ty| match (constructor, &*ty) {
            (Value::Primitive(p), TyKind::Primitive(q, made)) if p == q => Some(made.clone()),
            (Value::Fn(f), TyKind::Nominal(Nominal { instance: Some(i), .. })) if i.f == *f => {
                Some(i.args.clone())
            }
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
