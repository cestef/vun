//! Counting the expressions of a body.

use vun_hir::tree::{ExprId, ExprKind, Hir, NsId};
use vun_hir::visit::Visitor;

/// Counts the expressions of a body, as the typer reaches them: not into nested functions or
/// namespaces, which are typed on their own, but into closures, typed as part of the body.
pub(super) struct Exprs(pub(super) usize);

impl Visitor for Exprs {
    fn namespace(&mut self, _: &Hir, _: NsId) {}

    fn expr(&mut self, hir: &Hir, expr: ExprId) {
        self.0 += 1;
        match &hir.exprs[expr].kind {
            ExprKind::Fn { .. } => {}
            ExprKind::Closure { func, .. } => {
                vun_hir::visit::walk_expr(self, hir, expr);
                vun_hir::visit::walk_expr(self, hir, *func);
            }
            _ => vun_hir::visit::walk_expr(self, hir, expr),
        }
    }
}
