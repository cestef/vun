//! Blocks and statements: declarations bind locals, a pattern that always matches destructures,
//! expressions run for their effects, and the tail is the block's value.

use vun_hir::tree::{BlockId as HirBlock, PatKind, Stmt as HirStmt};

use super::{Builder, Unbuilt};
use crate::body::{Operand, Rvalue, TerminatorKind};
use crate::place::Place;

impl Builder<'_, '_, '_> {
    /// The statements of `block`, then the value of its tail, or `()` without one; the
    /// statements it deferred run after the value.
    pub(super) fn block(&mut self, block: HirBlock) -> Result<Operand, Unbuilt> {
        let hir = &self.env.parsed.hir;
        let depth = self.defers.len();
        self.stmts(&hir.blocks[block].stmts)?;
        let value = match hir.blocks[block].tail {
            Some(tail) => self.operand(tail)?,
            None => Self::unit(),
        };
        // The block ends where its value is written, or at the block.
        let end = hir.blocks[block].tail.map_or(hir.blocks[block].origin, |t| hir.exprs[t].origin);
        self.unwind(depth, end)?;
        self.defers.truncate(depth);
        Ok(value)
    }

    pub(super) fn stmt(&mut self, stmt: &HirStmt) -> Result<(), Unbuilt> {
        let hir = &self.env.parsed.hir;
        match stmt {
            HirStmt::Decl(d) => {
                let decl = &hir.decls[*d];
                match (&hir.pats[decl.pat].kind, decl.value) {
                    (PatKind::Bind(binder), value) => {
                        // Without a value, the local starts uninitialized.
                        let ty = value.and_then(|v| self.type_of(v));
                        let value = value.map(|v| self.operand(v)).transpose()?;
                        let local = self.bind(*binder);
                        self.locals[local].ty = ty;
                        if let Some(value) = value {
                            self.assign(Place::of(local), Rvalue::Use(value), decl.origin);
                        }
                    }
                    // A pattern that always matches, such as `(a, b) := pair`.
                    (_, Some(value)) => {
                        let matched = self.declared(value)?;
                        let never = self.fresh(decl.origin);
                        self.test(decl.pat, &matched, never)?;
                        self.blocks[never].term.kind = TerminatorKind::Unreachable;
                    }
                    (_, None) => return Err(Unbuilt { at: decl.origin }),
                }
            }
            HirStmt::Expr(e) => {
                self.operand(*e)?;
            }
            HirStmt::Block(inner) => {
                self.block(*inner)?;
            }
            HirStmt::Assign { op, place, value, origin } => self.store(*op, *place, *value, *origin)?,
        }
        Ok(())
    }
}
