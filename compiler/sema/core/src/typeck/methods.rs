//! Methods through impls (spec 02, section 3), and names declared twice for one type: by two
//! impls a file sees, or by an impl and the type itself (`impl.collision`).

use vun_diag::{Diagnostic, Msg, errors};
use vun_hir::tree::ExprId;
use vun_intern::Symbol;
use vun_origin::OriginId;
use vun_sema_ty::call::Instance;
use vun_sema_ty::{Ty, TyKind, Value};

use super::{Known, Typer};
use crate::eval::Call;
use crate::member::{Found, MemberType, Members};
use crate::witness::Method;

impl Typer<'_, '_, '_> {
    /// The struct declaring the method `name` that `ty` has through an impl (spec 02, section 3);
    /// two impls declaring it collide.
    pub(super) fn method(&mut self, expr: ExprId, ty: Ty, name: Symbol) -> Option<Ty> {
        let methods = self.env().methods(ty, name);
        let method = methods.first()?.clone();
        if methods.len() > 1 {
            self.collision(expr, name, methods.iter().map(|m| m.decl.decl.origin).collect());
        }
        self.check_method(expr, name, &method);
        Some(method.owner)
    }

    /// Checks `method`, which `expr` uses by the name `name`: that it is visible there, and that
    /// the witness it is found through, when made for this use, meets its interface's
    /// requirements (spec 02, section 3), reported here with the impl labeled.
    pub(super) fn check_method(&mut self, expr: ExprId, name: Symbol, method: &Method) {
        self.check_visible(expr, name, &method.visible);
        let witness = &method.witness;
        let (Some(requirements), true) = (witness.requirements, witness.made()) else { return };
        let origin = self.hir().exprs[expr].origin;
        for unmet in witness.unmet(self.env().cx).iter() {
            let d = unmet.diagnostic(requirements).at(origin, None);
            self.diag.error(d.label(witness.decl.attr, Some(Msg::new().text("the impl is here"))));
        }
    }

    /// Reports `name` at `expr` as declared at each of `origins` (`impl.collision`).
    pub(super) fn collision(&mut self, expr: ExprId, name: Symbol, origins: Vec<OriginId>) {
        let msg = Msg::new().code(name.as_str()).text(" is declared more than once for this type");
        let mut d = Diagnostic::error(errors::IMPL_COLLISION, msg).at(self.hir().exprs[expr].origin, None);
        for origin in origins {
            d = d.label(origin, Some(Msg::new().text("declared here")));
        }
        let note = "a call names the member, so the type may have only one of that name";
        self.diag.error(d.note(note).help("rename all but one of them, or remove the impl it comes from"));
    }

    /// The member `name` of a value whose type is known by an interface: a constrained `$`
    /// parameter in its generic body, which may use only what the interface requires, or a
    /// frame, which answers its interface's requirements itself (spec 06, section 1) and has the
    /// methods of its impls.
    pub(super) fn required_member(&mut self, expr: ExprId, ty: Ty, name: Symbol) -> Option<Known> {
        let interface = match &*ty {
            TyKind::Param(param) => &param.interface,
            TyKind::Frame(frame) => &frame.interface,
            _ => return None,
        };
        let call = Instance { f: interface.clone(), args: vec![Value::Type(ty)] };
        let requirements = self.env().cx.get::<Call>(&call).ok()?.as_type()?;
        let members = self.env().cx.get::<Members>(&requirements)?;
        match members.find(name) {
            Found::Decl(_) => {
                self.finding().methods.insert(expr, (requirements, name));
                self.env().cx.get::<MemberType>(&(requirements, name)).ok()
            }
            Found::Unknown => None,
            // A frame has the methods of its impls too, as any value.
            _ if matches!(&*ty, TyKind::Frame(_))
                && let Some(owner) = self.method(expr, ty, name) =>
            {
                self.finding().methods.insert(expr, (owner, name));
                self.env().cx.get::<MemberType>(&(owner, name)).ok()
            }
            _ => {
                self.no_member(self.hir().exprs[expr].origin, ty, name);
                None
            }
        }
    }
}
