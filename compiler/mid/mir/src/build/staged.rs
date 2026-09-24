//! `$for` and `$if` in a body (spec 03, section 1): a `$if` lowers the branch its known
//! condition takes, among statements or as a value, a `$for` its body once per item, with the binder bound and the step's own
//! types. A condition or items not known here leave the body unbuilt.

use vun_hir::tree::{Entry, ExprId, List, PatKind, Staged, Stmt as HirStmt};
use vun_origin::OriginId;
use vun_sema::env::Selected;
use vun_sema::eval::Staging;
use vun_sema::item::Node;
use vun_sema::ty::Value;
use vun_sema::typeck::Step;

use super::{Builder, Unbuilt};
use crate::body::Operand;

impl Builder<'_, '_, '_> {
    /// The statements of `list`, generated ones included.
    pub(super) fn stmts(&mut self, list: &List<HirStmt>) -> Result<(), Unbuilt> {
        for entry in list {
            match entry {
                Entry::One(stmt) => self.stmt(stmt)?,
                Entry::Staged(staged) => self.unroll(staged)?,
            }
        }
        Ok(())
    }

    /// `$if` as a value: the branch its conditions take, known here.
    pub(super) fn selected(&mut self, branch: ExprId, origin: OriginId) -> Result<Operand, Unbuilt> {
        match self.env().select(branch).map_err(|_| Unbuilt { at: origin })? {
            Selected::Block(block) => self.block(block),
            Selected::Expr(otherwise) => self.operand(otherwise),
            Selected::Unit => Ok(Self::unit()),
        }
    }

    fn unroll(&mut self, staged: &Staged<HirStmt>) -> Result<(), Unbuilt> {
        let hir = &self.env.parsed.hir;
        match staged {
            Staged::If { cond, then, otherwise, origin } => {
                let holds = self.env().eval(*cond).and_then(|c| self.env().holds(&c));
                self.stmts(if holds.map_err(|_| Unbuilt { at: *origin })? { then } else { otherwise })
            }
            Staged::For { pat, iter, body, origin } => {
                let unbuilt = Unbuilt { at: *origin };
                let items = self.env().eval(*iter).ok().and_then(Value::items).ok_or(unbuilt)?;
                let PatKind::Bind(binder) = hir.pats[*pat].kind else { return Err(unbuilt) };
                let binder = Node { id: binder, origin: hir.binders[binder].origin };
                for (index, value) in items.into_iter().enumerate() {
                    let env = self.env().clone().staging(&[Staging { binder, value }]);
                    let outer = self.step.replace(env);
                    self.path.push(Step { at: *pat, index });
                    let built = self.stmts(body);
                    self.path.pop();
                    self.step = outer;
                    built?;
                }
                Ok(())
            }
        }
    }
}
