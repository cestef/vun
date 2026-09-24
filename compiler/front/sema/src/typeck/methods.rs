//! Methods through impls (spec 02, section 3), and names declared twice for one type: by two
//! impls a file sees, or by an impl and the type itself (`impl.collision`).

use overt_diag::{Diagnostic, Msg, errors};
use overt_hir::tree::ExprId;
use overt_intern::Symbol;
use overt_origin::OriginId;

use super::{Known, Typer};
use crate::eval::{Call, Instance};
use crate::member::{Found, MemberType, Members};
use crate::ty::{Ty, TyKind, Value};

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

    /// The member `name` of a constrained `$` parameter's value in its generic body: a
    /// declaration of the interface's requirement struct for it, which is all the body may use.
    pub(super) fn param_member(&mut self, expr: ExprId, ty: Ty, name: Symbol) -> Option<Known> {
        let TyKind::Param(param) = &*ty else { return None };
        let call = Instance { f: param.interface.clone(), args: vec![Value::Type(ty)] };
        let requirements = self.env().cx.get::<Call>(&call).ok()?.as_type()?;
        let members = self.env().cx.get::<Members>(&requirements)?;
        match members.find(name) {
            Found::Decl(_) => {
                self.methods.insert(expr, (requirements, name));
                self.env().cx.get::<MemberType>(&(requirements, name)).ok()
            }
            Found::Unknown => None,
            _ => {
                self.no_member(self.hir().exprs[expr].origin, ty, name);
                None
            }
        }
    }
}
