//! Blocks and their statements: declarations, expressions, nested blocks and assignments, whose
//! value is checked against the place's type.

use vun_grammar::TokenKind;
use vun_hir::tree::{BlockId, Entry, ExprKind, Stmt};
use vun_hir::visit::{self, Visitor};

use super::{Known, Stage, Typer};
use crate::ty::{Ty, TyKind};

impl Typer<'_, '_, '_> {
    /// Types the statements of `block`, and gives its value: the tail, typed where `expected` is
    /// wanted, `()` without one, or nothing known when a statement leaves the block.
    pub(super) fn block_value(&mut self, block: BlockId, expected: Option<Ty>) -> Option<Known> {
        let hir = self.hir();
        let b = &hir.blocks[block];
        visit::walk_list(self, hir, &b.stmts, &mut |v, s| v.stmt(s));
        if let Some(tail) = b.tail {
            return self.typed_as(tail, expected);
        }
        let leaves = b.stmts.iter().any(|s| {
            matches!(s, Entry::One(Stmt::Expr(e)) if matches!(
                hir.exprs[*e].kind,
                ExprKind::Return(_) | ExprKind::Break { .. } | ExprKind::Continue { .. }
            ))
        });
        (!leaves).then(|| Known { ty: TyKind::unit(), stage: Stage::Compile })
    }

    fn stmt(&mut self, stmt: &Stmt) {
        let hir = self.hir();
        match stmt {
            Stmt::Decl(d) => self.decl(hir, *d),
            Stmt::Expr(e) => {
                self.typed(*e);
            }
            Stmt::Block(b) => {
                self.block_value(*b, None);
            }
            // `x op= y` narrows back to the type of `x` (spec 04, rule 9): a trap site, not a
            // type error.
            Stmt::Assign { op, place, value, .. } => {
                let place = self.typed(*place);
                let plain = *op == TokenKind::Eq;
                let found = self.typed_as(*value, place.filter(|_| plain).map(|k| k.ty));
                if let (true, Some(place), Some(found)) = (plain, place, found) {
                    self.expect(hir.exprs[*value].origin, found, place.ty, None);
                }
            }
        }
    }
}
