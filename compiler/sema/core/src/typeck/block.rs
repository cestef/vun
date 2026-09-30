//! Blocks and their statements: declarations, expressions, nested blocks and assignments, whose
//! value is checked against the place's type.

use vun_desugar::COMPOUND;
use vun_hir::tree::{BlockId, ExprKind, Stmt};
use vun_hir::visit::{self, Visitor};
use vun_sema_ty::{Ty, TyKind};

use super::{Known, Stage, Typer};

impl Typer<'_, '_, '_> {
    /// Types the statements of `block`, and gives its value: the tail, typed where `expected` is
    /// wanted, `()` without one, or nothing known when a statement leaves the block.
    pub(super) fn block_value(&mut self, block: BlockId, expected: Option<Ty>) -> Option<Known> {
        let hir = self.hir();
        let b = &hir.blocks[block];
        // Narrowings made after a statement last to the end of the block.
        self.narrowing(&Vec::new(), |t| {
            visit::walk_list(t, hir, &b.stmts, &mut |v, s| {
                v.stmt(s);
                v.after(s);
            });
            match b.tail {
                Some(tail) => t.typed_as(tail, expected),
                None if t.leaves(block) => t.never(),
                None => Some(Known { ty: TyKind::unit(), stage: Stage::Compile }),
            }
        })
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
            // Its names are resolved; it computes nothing.
            Stmt::Spread(_) => {}
            // `x op= y` narrows back to the type of `x` (spec 04, rule 9): a trap site, not a
            // type error.
            Stmt::Assign { op, place, value, .. } => {
                if let Some(row) = COMPOUND.iter().find(|row| row.token == *op) {
                    self.compound(*place, row.op, *value);
                } else if matches!(hir.exprs[*place].kind, ExprKind::Hole) {
                    // `_ = e` discards `e`: the hole holds what it is given.
                    let found = self.typed(*value);
                    self.typed_as(*place, found.map(|k| k.ty));
                } else {
                    let place_known = self.typed(*place);
                    let found = self.typed_as(*value, place_known.map(|k| k.ty));
                    if let (Some(place), Some(found)) = (place_known, found) {
                        self.expect(hir.exprs[*value].origin, found, place.ty, None);
                    }
                }
            }
        }
    }
}
