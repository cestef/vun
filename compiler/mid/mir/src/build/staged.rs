//! `$for` and `$if` in a body (spec 03, section 1): a `$if` lowers the branch its known
//! condition takes, among statements or as a value, a `$for` its body once per item, with the binder bound and the step's own
//! types. A condition or items not known here leave the body unbuilt.

use std::sync::Arc;

use vun_hir::tree::{Cond, Entry, ExprId, List, PatKind, Staged, Stmt as HirStmt};
use vun_origin::OriginId;
use vun_sema::env::Selected;
use vun_sema::typed::Step;
use vun_sema_ty::Value;
use vun_sema_ty::call::Staging;
use vun_sema_ty::item::Node;

use super::{Builder, Unbuilt};
use crate::Operand;

impl Builder<'_, '_, '_> {
    /// The statements of `list`, generated ones included.
    pub(super) fn stmts(&mut self, list: &List<HirStmt>) -> Result<(), Unbuilt> {
        self.entries(list, &mut |b, stmt| b.stmt(stmt))
    }

    /// Runs `each` on every entry of `list`, those `$for` and `$if` generate included, each in
    /// its step.
    pub(super) fn entries<T>(
        &mut self,
        list: &List<T>,
        each: &mut dyn FnMut(&mut Self, &T) -> Result<(), Unbuilt>,
    ) -> Result<(), Unbuilt> {
        for entry in list {
            match entry {
                Entry::One(item) => each(self, item)?,
                Entry::Staged(staged) => self.unroll(staged, each)?,
            }
        }
        Ok(())
    }

    /// `$if` as a value: the branch its conditions take, known here.
    pub(super) fn selected(&mut self, branch: ExprId, origin: OriginId) -> Result<Operand, Unbuilt> {
        match self.env().select(branch).map_err(|_| Unbuilt::at(origin))? {
            Selected::Block(block) => self.block(block),
            Selected::Tested { block, pat, bound } => {
                self.in_step(Step { at: pat, index: 0 }, &bound, |b| b.block(block))
            }
            Selected::Expr(otherwise) => self.operand(otherwise),
            Selected::Unit => Ok(Self::unit()),
        }
    }

    fn unroll<T>(
        &mut self,
        staged: &Staged<T>,
        each: &mut dyn FnMut(&mut Self, &T) -> Result<(), Unbuilt>,
    ) -> Result<(), Unbuilt> {
        let hir = &self.env.parsed.hir;
        match staged {
            Staged::If { cond, then, otherwise, origin } => {
                match (self.env().tested(cond).map_err(|_| Unbuilt::at(*origin))?, cond) {
                    (None, _) => self.entries(otherwise, each),
                    (Some(_), Cond::Expr(_)) => self.entries(then, each),
                    // A test's branch is the step typing typed it as, with what the test binds.
                    (Some(bound), Cond::Test { pat, .. }) => {
                        self.in_step(Step { at: *pat, index: 0 }, &bound, |b| b.entries(then, each))
                    }
                }
            }
            Staged::For { pat, iter, body, origin } => {
                let unbuilt = move || Unbuilt::at(*origin);
                let items = self.env().eval(*iter).ok().and_then(Value::items).ok_or_else(unbuilt)?;
                // `_` binds nothing: the body is repeated as written.
                let binder = match hir.pats[*pat].kind {
                    PatKind::Bind(binder) => Some(Node { id: binder, origin: hir.binders[binder].origin }),
                    PatKind::Hole => None,
                    _ => return Err(unbuilt()),
                };
                for (index, value) in items.into_iter().enumerate() {
                    let staged: Vec<Staging> =
                        binder.iter().map(|&binder| Staging { binder, value: value.clone() }).collect();
                    self.in_step(Step { at: *pat, index }, &staged, |b| b.entries(body, each))?;
                }
                Ok(())
            }
        }
    }

    /// What `f` builds as the step `step`, under the bindings `staged`, with the types typing
    /// found for it.
    fn in_step<R>(
        &mut self,
        step: Step,
        staged: &[Staging],
        f: impl FnOnce(&mut Self) -> Result<R, Unbuilt>,
    ) -> Result<R, Unbuilt> {
        let env = self.env().clone().staging(staged);
        self.path.push(step);
        let outer = self.step.replace(env.typed_by(self.typed().map(|t| Arc::clone(&t.found))));
        let built = f(self);
        self.path.pop();
        self.step = outer;
        built
    }
}
