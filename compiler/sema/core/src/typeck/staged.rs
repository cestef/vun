//! `$for` and `$if` in a body (spec 03, section 1): a `$if` whose condition is known types the
//! branch taken, among statements or as a value, a test's branch as a step with what it binds; a `$for` whose items are known types its body once per item, with the binder
//! bound, each step's types kept apart ([`Step`]). Otherwise the entries are typed once, with
//! nothing bound.

use std::sync::Arc;

use vun_diag::DiagCtxt;
use vun_hir::tree::{Cond, ExprId, ExprKind, Hir, PatKind, Staged};
use vun_hir::visit::{walk_list, walk_staged};
use vun_sema_ty::call::Staging;
use vun_sema_ty::item::Node;
use vun_sema_ty::{TyKind, Value};

use super::{Found, Known, Stage, Step, TypedBody, Typer};
use crate::env::{Env, Selected};

impl<'a, 'db> Typer<'_, 'a, 'db> {
    pub(super) fn unroll<T>(&mut self, hir: &Hir, staged: &Staged<T>, entry: &mut dyn FnMut(&mut Self, &T)) {
        match staged {
            Staged::If { cond, then, otherwise, .. } => {
                // What a condition reads is typed; a test's pattern gives its binders the types of
                // the parts they take.
                let (at, known) = match cond {
                    Cond::Expr(c) => (*c, self.typed(*c)),
                    Cond::Test { pat, value } => {
                        let known = self.typed(*value);
                        self.test(*pat, self.matched(*value, known));
                        (*value, known)
                    }
                };
                let tested = self.evaluating().tested(cond);
                match (self.ran(at, known, tested), cond) {
                    (Ok(None), _) => walk_list(self, hir, otherwise, entry),
                    (Ok(Some(_)), Cond::Expr(_)) => walk_list(self, hir, then, entry),
                    // A test's branch is a step of its own, typed with what the test binds.
                    (Ok(Some(bound)), Cond::Test { pat, .. }) => {
                        self.stepped(Step { at: *pat, index: 0 }, &bound, |t| walk_list(t, hir, then, entry));
                    }
                    (Err(_), _) => self.undecided(|t| walk_staged(t, hir, staged, entry)),
                }
            }
            Staged::For { pat, iter, body, .. } => {
                let items = self.evaluating().eval(*iter);
                let items = self.reported(*iter, items).ok().and_then(Value::items);
                // `_` binds nothing: each step is the body as written.
                let binder = match hir.pats[*pat].kind {
                    PatKind::Bind(binder) => Some(binder),
                    PatKind::Hole => None,
                    _ => {
                        self.undecided(|t| walk_staged(t, hir, staged, entry));
                        return;
                    }
                };
                let Some(items) = items else {
                    self.undecided(|t| walk_staged(t, hir, staged, entry));
                    return;
                };
                let item = self.typed(*iter).and_then(|k| self.iteration(k.ty)).map(|(_, item)| item);
                for (index, value) in items.into_iter().enumerate() {
                    // A step's binder has the type of its item alone, as a bound `$` parameter.
                    let ty = value.ty().or(item);
                    let staged: Vec<Staging> = binder
                        .map(|b| Staging { binder: Node { id: b, origin: hir.binders[b].origin }, value })
                        .into_iter()
                        .collect();
                    self.stepped(Step { at: *pat, index }, &staged, |t| {
                        if let (Some(binder), Some(ty)) = (binder, ty) {
                            t.bind(binder, ty);
                        }
                        walk_list(t, hir, body, entry);
                    });
                }
            }
        }
    }

    /// `$operand`: a `$if` selects its branch, code of type `Code(T)` is spliced as a value of
    /// `T`, and anything else is computed at compile time.
    pub(super) fn comptime(&mut self, expr: ExprId, operand: ExprId) -> Option<Known> {
        if matches!(self.hir().exprs[operand].kind, ExprKind::If { .. }) {
            return self.selected(expr, operand);
        }
        let outer = std::mem::replace(&mut self.stage, Stage::Compile);
        // A `$` form inside runs as part of this one, never on its own.
        let known = self.undecided(|t| t.typed(operand));
        self.stage = outer;
        // Where it surely runs, it runs once the body is typed, with the body's types.
        if self.undecided == 0 {
            self.finding().evaluations.push(expr);
        }
        match known.and_then(|k| k.ty.quoted()) {
            Some(ty) => Some(Known { ty, stage: self.stage }),
            None => known,
        }
    }

    /// `$if` as a value: the branch its known conditions take, typed as the value; with a
    /// condition not known here, the `if` is typed as a plain one.
    fn selected(&mut self, expr: ExprId, branch: ExprId) -> Option<Known> {
        let expected = self.expected(expr);
        self.tests(branch);
        let selected = self.evaluating().select(branch);
        match self.reported(branch, selected) {
            Ok(Selected::Block(block)) => self.block_value(block, expected),
            Ok(Selected::Tested { block, pat, bound }) => {
                let mut value = None;
                self.stepped(Step { at: pat, index: 0 }, &bound, |t| value = t.block_value(block, expected));
                value
            }
            Ok(Selected::Expr(otherwise)) => self.typed_as(otherwise, expected),
            Ok(Selected::Unit) => Some(Known { ty: TyKind::unit(), stage: Stage::Compile }),
            Err(_) => self.undecided(|t| t.typed_as(branch, expected)),
        }
    }

    /// Types what the tests of the `if` chain `expr` read, their patterns giving their binders
    /// the types of the parts they take, so that selecting a branch evaluates them.
    fn tests(&mut self, mut expr: ExprId) {
        let hir = self.hir();
        while let ExprKind::If { cond, otherwise, .. } = &hir.exprs[expr].kind {
            if let Cond::Test { pat, value } = cond {
                let known = self.typed(*value);
                self.test(*pat, self.matched(*value, known));
            }
            let Some(next) = otherwise else { return };
            expr = *next;
        }
    }

    /// `f` run on code that does not run on its own here: under a `$if` or `$for` not decided
    /// here, or inside a `$` form, which runs it.
    fn undecided<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        self.undecided += 1;
        let result = f(self);
        self.undecided -= 1;
        result
    }

    /// Types what `f` types as the step `step`, under the bindings `staged`, kept apart.
    fn stepped(&mut self, step: Step, staged: &[Staging], f: impl FnOnce(&mut Self)) {
        let env = self.env().clone().staging(staged);
        let typed = self.step(env, f);
        self.finding().steps.insert(step, Arc::new(typed));
    }

    /// Types what `f` types in `env`, apart from the rest of the body; reports stay with it.
    fn step(&mut self, env: Env<'a, 'db>, f: impl FnOnce(&mut Self)) -> TypedBody {
        let staged = env.staged().to_vec();
        let outer = self.step.replace(env);
        let found = std::mem::replace(&mut self.found, Arc::new(Found { staged, ..Found::default() }));
        // What the step binds is its own: the binders it declares have their types there.
        let locals = self.locals.clone();
        f(self);
        let stepped = std::mem::replace(&mut self.locals, locals);
        let binders = stepped
            .into_iter()
            .filter(|(b, k)| self.locals.get(b) != Some(k))
            .filter_map(|(b, k)| Some((b, k.ok()?.ty)));
        let typed = std::mem::replace(&mut self.found, found).body(binders.collect(), DiagCtxt::new(), 0);
        self.step = outer;
        typed
    }
}
