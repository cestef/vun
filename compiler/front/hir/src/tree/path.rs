//! Paths: a name followed by member names, `a.b.c`.

use vun_intern::Symbol;

use super::{ExprId, ExprKind, Hir, Name};

impl Hir {
    /// The names of the path `expr`, and the expression of its first name; `None` when `expr` is
    /// not a path.
    #[must_use]
    pub fn path(&self, expr: ExprId) -> Option<(ExprId, Vec<Symbol>)> {
        match self.exprs[expr].kind {
            ExprKind::Name(name) => Some((expr, vec![name])),
            ExprKind::Member { object, name: Name::Plain(name) } => {
                let (root, mut names) = self.path(object)?;
                names.push(name);
                Some((root, names))
            }
            _ => None,
        }
    }
}
