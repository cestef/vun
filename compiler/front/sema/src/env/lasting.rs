//! What `&e` makes of `e`: a borrow type when `e` denotes a type (type syntax mirrors value
//! syntax, spec 01), and a `&static` borrow when it lives as long as the program (the row
//! `static` of the table `borrows`, spec 05, section 4).

use vun_hir::tree::{Entry, ExprId, ExprKind};

use super::Env;
use crate::item::Site;

impl Env<'_, '_> {
    /// Whether what `expr` denotes lives as long as the program: a literal, a tuple, array or
    /// record made of such, an immutable top-level binding, or a part of one of these reached by
    /// a field or an index, which is in the same storage.
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
            ExprKind::Member { object, .. } => self.immutable_member(expr) || self.lasting(*object),
            ExprKind::Index { object, .. } => self.lasting(*object),
            _ => false,
        }
    }

    /// Whether `expr` denotes a type, such as `u8` or `[u8]`.
    #[must_use]
    pub fn denotes_type(&self, expr: ExprId) -> bool {
        self.eval(expr).ok().and_then(|v| v.as_type()).is_some()
    }

    /// Whether the name `expr` leads to a top-level binding that is not `mut`.
    fn immutable_member(&self, expr: ExprId) -> bool {
        matches!(self.resolve(expr), Ok(Site::Decl(decl)) if !decl.mutable(self.cx))
    }
}
