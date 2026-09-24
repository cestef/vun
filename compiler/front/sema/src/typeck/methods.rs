//! Methods through impls (spec 02, section 3), and names declared twice for one type: by two
//! impls a file sees, or by an impl and the type itself (`impl.collision`).

use overt_diag::{Diagnostic, Msg, errors};
use overt_hir::tree::ExprId;
use overt_intern::Symbol;
use overt_origin::OriginId;

use super::Typer;
use crate::ty::Ty;

impl Typer<'_, '_, '_> {
    /// The struct declaring the method `name` that `ty` has through an impl (spec 02, section 3);
    /// two impls declaring it collide.
    pub(super) fn method(&mut self, expr: ExprId, ty: Ty, name: Symbol) -> Option<Ty> {
        let methods = self.env().methods(ty, name);
        let method = methods.first()?.clone();
        if methods.len() > 1 {
            self.collision(expr, name, methods.iter().map(|m| m.decl.decl.origin).collect());
        }
        self.check_visible(expr, name, &method.visible);
        Some(method.owner)
    }

    /// Reports `name` at `expr` as declared at each of `origins` (`impl.collision`).
    pub(super) fn collision(&mut self, expr: ExprId, name: Symbol, origins: Vec<OriginId>) {
        let msg = Msg::new().code(name.as_str()).text(" is declared more than once for this type");
        let mut d = Diagnostic::error(errors::IMPL_COLLISION, msg).at(self.hir().exprs[expr].origin, None);
        for origin in origins {
            d = d.label(origin, Some(Msg::new().text("declared here")));
        }
        self.diag.error(d);
    }
}
