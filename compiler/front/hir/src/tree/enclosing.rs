//! What encloses an expression: the closure a function literal belongs to, and the function
//! literal whose body holds an expression.

use super::{ExprId, ExprKind, Hir};
use crate::visit::{self, Visitor};

impl Hir {
    /// The closure whose function is the literal `func`, if it is one.
    #[must_use]
    pub fn closure_of(&self, func: ExprId) -> Option<ExprId> {
        self.exprs.iter().find_map(|(e, expr)| match expr.kind {
            ExprKind::Closure { func: f, .. } if f == func => Some(e),
            _ => None,
        })
    }

    /// The innermost function literal whose body holds `expr`, closures included.
    #[must_use]
    pub fn enclosing_function(&self, expr: ExprId) -> Option<ExprId> {
        let mut finder = Finder { target: expr, functions: Vec::new(), found: None };
        finder.namespace(self, self.root?);
        finder.found
    }
}

/// Walks the tree, keeping the function literals around the current node.
struct Finder {
    target: ExprId,
    functions: Vec<ExprId>,
    found: Option<ExprId>,
}

impl Visitor for Finder {
    fn expr(&mut self, hir: &Hir, expr: ExprId) {
        if self.found.is_some() {
            return;
        }
        if expr == self.target {
            self.found = self.functions.last().copied();
            return;
        }
        let function = matches!(hir.exprs[expr].kind, ExprKind::Fn { .. });
        if function {
            self.functions.push(expr);
        }
        visit::walk_expr(self, hir, expr);
        if function {
            self.functions.pop();
        }
    }
}
