//! Whether a function literal suspends (spec 06, section 1): its body holds a `yield`, outside
//! the functions nested in it, closures included.

use super::{ExprId, ExprKind, Hir};
use crate::visit::{self, Visitor};

impl Hir {
    /// Whether the function literal `func` is a coroutine function.
    #[must_use]
    pub fn suspends(&self, func: ExprId) -> bool {
        let ExprKind::Fn { body, .. } = &self.exprs[func].kind else { return false };
        let mut finder = Yields(false);
        finder.block(self, *body);
        finder.0
    }
}

struct Yields(bool);

impl Visitor for Yields {
    fn expr(&mut self, hir: &Hir, expr: ExprId) {
        match hir.exprs[expr].kind {
            ExprKind::Yield(_) => self.0 = true,
            ExprKind::Fn { .. } => {}
            _ if !self.0 => visit::walk_expr(self, hir, expr),
            _ => {}
        }
    }
}
