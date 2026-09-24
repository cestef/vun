//! `$for` and `$if` in a body (spec 03, section 1): a `$if` whose condition is known types the
//! branch taken, among statements or as a value; a `$for` whose items are known types its body once per item, with the binder
//! bound, each step's types kept apart ([`Step`]). Otherwise the entries are typed once, with
//! nothing bound.

use std::mem::take;
use std::sync::Arc;

use overt_diag::DiagCtxt;
use overt_hir::tree::{ExprId, ExprKind, Hir, PatKind, Staged};
use overt_hir::visit::{walk_list, walk_staged};

use super::{Known, Stage, Step, TypedBody, Typer};
use crate::env::{Env, Selected};
use crate::eval::Staging;
use crate::item::Node;
use crate::ty::{TyKind, Value};

impl<'a, 'db> Typer<'_, 'a, 'db> {
    pub(super) fn unroll<T>(&mut self, hir: &Hir, staged: &Staged<T>, entry: &mut dyn FnMut(&mut Self, &T)) {
        match staged {
            Staged::If { cond, then, otherwise, .. } => {
                self.typed(*cond);
                match self.env().eval(*cond).and_then(|c| self.env().holds(&c)) {
                    Ok(holds) => walk_list(self, hir, if holds { then } else { otherwise }, entry),
                    Err(_) => walk_staged(self, hir, staged, entry),
                }
            }
            Staged::For { pat, iter, body, .. } => {
                let items = self.env().eval(*iter).ok().and_then(Value::items);
                let (Some(items), PatKind::Bind(binder)) = (items, &hir.pats[*pat].kind) else {
                    walk_staged(self, hir, staged, entry);
                    return;
                };
                let item = self.typed(*iter).and_then(|k| self.item(k.ty));
                let node = Node { id: *binder, origin: hir.binders[*binder].origin };
                for (index, value) in items.into_iter().enumerate() {
                    // A step's binder has the type of its item alone, as a bound `$` parameter.
                    let ty = value.ty().or(item);
                    let env = self.env().clone().staging(&[Staging { binder: node, value }]);
                    let typed = self.step(env, |t| {
                        if let Some(ty) = ty {
                            t.bind(*binder, ty);
                        }
                        walk_list(t, hir, body, entry);
                    });
                    self.steps.insert(Step { at: *pat, index }, Arc::new(typed));
                }
            }
        }
    }

    /// `$operand`: a `$if` selects its branch, anything else is computed at compile time.
    pub(super) fn comptime(&mut self, expr: ExprId, operand: ExprId) -> Option<Known> {
        if matches!(self.hir().exprs[operand].kind, ExprKind::If { .. }) {
            return self.selected(expr, operand);
        }
        let outer = std::mem::replace(&mut self.stage, Stage::Compile);
        let known = self.typed(operand);
        self.stage = outer;
        known
    }

    /// `$if` as a value: the branch its known conditions take, typed as the value; with a
    /// condition not known here, the `if` is typed as a plain one.
    fn selected(&mut self, expr: ExprId, branch: ExprId) -> Option<Known> {
        let expected = self.expected(expr);
        match self.env().select(branch) {
            Ok(Selected::Block(block)) => self.block_value(block, expected),
            Ok(Selected::Expr(otherwise)) => self.typed_as(otherwise, expected),
            Ok(Selected::Unit) => Some(Known { ty: TyKind::unit(), stage: Stage::Compile }),
            Err(_) => self.typed_as(branch, expected),
        }
    }

    /// Types what `f` types in `env`, apart from the rest of the body; reports stay with it.
    fn step(&mut self, env: Env<'a, 'db>, f: impl FnOnce(&mut Self)) -> TypedBody {
        let outer = self.step.replace(env);
        let saved = (take(&mut self.types), take(&mut self.methods), take(&mut self.steps));
        f(self);
        let typed = TypedBody {
            types: take(&mut self.types).into_iter().collect(),
            methods: take(&mut self.methods).into_iter().collect(),
            steps: take(&mut self.steps),
            diag: DiagCtxt::new(),
            exprs: 0,
        };
        (self.types, self.methods, self.steps) = saved;
        self.step = outer;
        typed
    }
}
