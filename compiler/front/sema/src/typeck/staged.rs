//! `$for` and `$if` in a body (spec 03, section 1): a `$if` whose condition is known types the
//! branch taken, among statements or as a value; a `$for` whose items are known types its body once per item, with the binder
//! bound, each step's types kept apart ([`Step`]). Otherwise the entries are typed once, with
//! nothing bound.

use std::sync::Arc;

use vun_diag::DiagCtxt;
use vun_hir::tree::{ExprId, ExprKind, Hir, PatKind, Staged};
use vun_hir::visit::{walk_list, walk_staged};

use super::{Found, Known, Stage, Step, TypedBody, Typer};
use crate::env::{Env, Selected};
use crate::eval::Staging;
use crate::item::Node;
use crate::ty::{TyKind, Value};

impl<'a, 'db> Typer<'_, 'a, 'db> {
    pub(super) fn unroll<T>(&mut self, hir: &Hir, staged: &Staged<T>, entry: &mut dyn FnMut(&mut Self, &T)) {
        match staged {
            Staged::If { cond, then, otherwise, .. } => {
                let known = self.typed(*cond);
                let env = self.evaluating();
                let holds = env.eval(*cond).and_then(|c| env.holds(&c));
                match self.ran(*cond, known, holds) {
                    Ok(holds) => walk_list(self, hir, if holds { then } else { otherwise }, entry),
                    Err(_) => self.undecided(|t| walk_staged(t, hir, staged, entry)),
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
                    let env = self.env().clone().staging(&staged);
                    let typed = self.step(env, |t| {
                        if let (Some(binder), Some(ty)) = (binder, ty) {
                            t.bind(binder, ty);
                        }
                        walk_list(t, hir, body, entry);
                    });
                    self.found.steps.insert(Step { at: *pat, index }, Arc::new(typed));
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
            self.found.evaluations.push(expr);
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
        let selected = self.env().select(branch);
        match self.reported(branch, selected) {
            Ok(Selected::Block(block)) => self.block_value(block, expected),
            Ok(Selected::Expr(otherwise)) => self.typed_as(otherwise, expected),
            Ok(Selected::Unit) => Some(Known { ty: TyKind::unit(), stage: Stage::Compile }),
            Err(_) => self.undecided(|t| t.typed_as(branch, expected)),
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

    /// Types what `f` types in `env`, apart from the rest of the body; reports stay with it.
    fn step(&mut self, env: Env<'a, 'db>, f: impl FnOnce(&mut Self)) -> TypedBody {
        let staged = env.staged().to_vec();
        let outer = self.step.replace(env);
        let found = std::mem::replace(&mut self.found, Found { staged, ..Found::default() });
        // What the step binds is its own: the binders it declares have their types there.
        let locals = self.locals.clone();
        f(self);
        let stepped = std::mem::replace(&mut self.locals, locals);
        let binders =
            stepped.into_iter().filter(|(b, k)| self.locals.get(b) != Some(k)).map(|(b, k)| (b, k.ty));
        let typed = std::mem::replace(&mut self.found, found).body(binders.collect(), DiagCtxt::new(), 0);
        self.step = outer;
        typed
    }
}
