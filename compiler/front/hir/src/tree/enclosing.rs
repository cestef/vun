//! What encloses an expression: the closure a function literal belongs to, the function
//! literal whose body holds an expression, and the literals that depend on compile-time bindings.

use std::collections::HashSet;

use super::{Entry, ExprId, ExprKind, Hir, Staged};
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

    /// The function literals whose code depends on bindings known only at a call: those with a
    /// `$` parameter or a hole in a parameter's type, those inside them, and those a `$for`
    /// generates.
    #[must_use]
    pub fn open_functions(&self) -> HashSet<ExprId> {
        let mut opener = Opener { depth: 0, open: HashSet::new() };
        if let Some(root) = self.root {
            opener.namespace(self, root);
        }
        opener.open
    }
}

impl Hir {
    /// Whether `expr` has a hole `_` in it.
    fn holds_hole(&self, expr: ExprId) -> bool {
        let mut holes = Holes(false);
        holes.expr(self, expr);
        holes.0
    }
}

struct Holes(bool);

impl Visitor for Holes {
    fn expr(&mut self, hir: &Hir, expr: ExprId) {
        self.0 |= matches!(hir.exprs[expr].kind, ExprKind::Hole);
        visit::walk_expr(self, hir, expr);
    }
}

/// Walks the tree, counting the open function literals and `$for` entries around the node.
struct Opener {
    depth: usize,
    open: HashSet<ExprId>,
}

impl Visitor for Opener {
    fn expr(&mut self, hir: &Hir, expr: ExprId) {
        let ExprKind::Fn { params, .. } = &hir.exprs[expr].kind else {
            return visit::walk_expr(self, hir, expr);
        };
        // A parameter whose type has a hole (`[_]u8`) takes it from each call, as a `$` one.
        let mut binders = Vec::new();
        let mut comptime = false;
        for param in params {
            match param {
                Entry::One(param) => {
                    param.pat.into_iter().for_each(|pat| hir.binders(pat, &mut binders));
                    comptime |= param.ty.is_some_and(|ty| hir.holds_hole(ty));
                }
                Entry::Staged(_) => comptime = true,
            }
        }
        let comptime = comptime || binders.iter().any(|&b| hir.binders[b].comptime);
        let opens = comptime || self.depth > 0;
        if opens {
            self.open.insert(expr);
            self.depth += 1;
        }
        visit::walk_expr(self, hir, expr);
        if opens {
            self.depth -= 1;
        }
    }

    fn staged<T>(&mut self, hir: &Hir, staged: &Staged<T>, entry: &mut dyn FnMut(&mut Self, &T)) {
        let binds = matches!(staged, Staged::For { .. });
        self.depth += usize::from(binds);
        visit::walk_staged(self, hir, staged, entry);
        self.depth -= usize::from(binds);
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
