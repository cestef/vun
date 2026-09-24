//! What lives as long as the program: a shared borrow of it is `&static` (the row `static` of the
//! table `borrows`, spec 05, section 4).

use vun_hir::tree::{Entry, ExprId, ExprKind};

use super::Env;
use crate::item::Site;

impl Env<'_, '_> {
    /// Whether what `expr` denotes lives as long as the program: a literal, a tuple, array or
    /// record made of such, or an immutable top-level binding.
    #[must_use]
    pub fn lasting(&self, expr: ExprId) -> bool {
        let hir = &self.parsed.hir;
        match &hir.exprs[expr].kind {
            ExprKind::Lit(_) | ExprKind::DotName(_) => true,
            ExprKind::Tuple(items) | ExprKind::Array(items) => {
                items.iter().all(|e| matches!(e, Entry::One(item) if self.lasting(*item)))
            }
            ExprKind::StructLit { fields, .. } => {
                fields.iter().all(|f| matches!(f, Entry::One(arg) if self.lasting(arg.value)))
            }
            ExprKind::Name(_) => self.immutable_member(expr),
            _ => false,
        }
    }

    /// Whether the name `expr` leads to a top-level binding that is not `mut`.
    fn immutable_member(&self, expr: ExprId) -> bool {
        matches!(self.resolve(expr), Ok(Site::Decl(decl)) if !decl.mutable(self.cx))
    }
}
