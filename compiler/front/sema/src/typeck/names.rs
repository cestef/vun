//! Names and members: a local, a declaration, a namespace, a member of a type's namespace or of
//! a value.

use vun_diag::{ChildKind, Diagnostic, Msg, errors};
use vun_hir::db::{Exports, Namespace};
use vun_hir::resolve::Res;
use vun_hir::tree::{ExprId, ExprKind};
use vun_intern::Symbol;
use vun_suggest::closest;

use super::{Known, Stage, Typer};
use crate::env::Env;
use crate::item::{Declared, Site};
use crate::member::{Found, MemberType, Members};
use crate::ty::{Signature, TyKind, Value};

impl Typer<'_, '_, '_> {
    /// A local by the type it was bound with; any other name by its declaration's type.
    pub(super) fn name(&mut self, expr: ExprId) -> Option<Known> {
        self.mutable_static(expr);
        if let Some(Res::Binder(b)) = self.env().parsed.resolution.names[expr] {
            if let Some(known) = self.locals.get(&b) {
                return Some(*known);
            }
            // A binder the environment binds, such as a `$for` binder: its value's type.
            if let Some(ty) = self.env().bound(b).and_then(Value::ty) {
                return Some(Known { ty, stage: Stage::Compile });
            }
        }
        self.site(self.env().resolve(expr).ok()?)
    }

    /// A member of a namespace, of a type's namespace, or of a value: a field, or a declaration of
    /// its type, called as a method.
    pub(super) fn member(&mut self, expr: ExprId, object: ExprId, name: Symbol) -> Option<Known> {
        let known = self.typed(object);
        // Only a path names a namespace or a type, so nothing else is evaluated here.
        if matches!(self.hir().exprs[object].kind, ExprKind::Name(_) | ExprKind::Member { .. }) {
            match self.env().eval(object) {
                Ok(Value::Namespace(ns)) => {
                    let Some(site) = self.env().cx.get::<Declared>(&(ns.clone(), name)) else {
                        self.absent(expr, object, &ns, name);
                        return None;
                    };
                    if let Site::Decl(decl) = &site
                        && let Some(visible) =
                            Env::of(self.env().cx, &decl.file).map(|e| e.decl_visible(decl.decl.id, name))
                    {
                        self.check_visible(expr, name, &visible);
                    }
                    return self.site(site);
                }
                Ok(Value::Type(ty)) => {
                    let members = self.env().cx.get::<Members>(&ty);
                    return match members.as_deref().map_or(Found::Unknown, |m| m.find(name)) {
                        Found::Decl(decl) => {
                            self.check_visible(expr, name, &decl.visible);
                            self.env().cx.get::<MemberType>(&(ty, name)).ok()
                        }
                        Found::Variant(v) if v.payload.is_empty() => {
                            Some(Known { ty, stage: Stage::Compile })
                        }
                        // A constructor is a function from the payload to the variant.
                        Found::Variant(v) => {
                            let params =
                                v.payload.iter().map(|p| p.as_ref().ok().copied()).collect::<Option<_>>()?;
                            Some(Known { ty: Signature::new(params, ty).ty(), stage: Stage::Compile })
                        }
                        Found::Field(_) => None,
                        Found::Absent | Found::Unknown => {
                            let owner = self.method(expr, ty, name)?;
                            self.env().cx.get::<MemberType>(&(owner, name)).ok()
                        }
                    };
                }
                _ => {}
            }
        }
        let known = known?;
        // `.` looks through borrows, and through owners lacking the member (spec 01, section 4).
        let known = Known { ty: self.env().reach(known.ty, name).1, ..known };
        match &*known.ty {
            TyKind::Record(_) => return self.record_field(expr, known.ty, name, known.stage),
            TyKind::Param(_) | TyKind::Frame(_) => return self.required_member(expr, known.ty, name),
            _ => {}
        }
        // A type without a namespace of its own, such as an integer, a tuple, an array or a
        // borrow, has only its impls' members. They are unknown for a nominal type whose members
        // are, and for a value declaring its own, a type or an interface not known here.
        let members = self.env().cx.get::<Members>(&known.ty);
        let unknown = if matches!(&*known.ty, TyKind::Nominal(_)) || known.ty.declares_members() {
            Found::Unknown
        } else {
            Found::Absent
        };
        match members.as_deref().map_or(unknown, |m| m.find(name)) {
            Found::Field(field) => {
                self.check_visible(expr, name, &field.visible);
                Some(Known { ty: *field.ty.as_ref().ok()?, stage: known.stage })
            }
            Found::Decl(decl) => {
                self.check_visible(expr, name, &decl.visible);
                // An impl declaring the type's own member again collides with it.
                let also = self.env().methods(known.ty, name);
                if !also.is_empty() {
                    let origins =
                        std::iter::once(decl.decl.decl.origin).chain(also.iter().map(|m| m.decl.decl.origin));
                    self.collision(expr, name, origins.collect());
                }
                self.methods.insert(expr, (known.ty, name));
                self.env().cx.get::<MemberType>(&(known.ty, name)).ok()
            }
            // A member missing from the type and its impls; a variant is reached through its
            // type, never through a value, so a value's lookup goes on to the impls.
            found @ (Found::Variant(_) | Found::Absent | Found::Unknown) => {
                let Some(owner) = self.method(expr, known.ty, name) else {
                    if !matches!(found, Found::Unknown) {
                        self.no_member(self.hir().exprs[expr].origin, known.ty, name);
                    }
                    return None;
                };
                self.methods.insert(expr, (owner, name));
                self.env().cx.get::<MemberType>(&(owner, name)).ok()
            }
        }
    }

    /// Reports `name`, used at `expr` on the namespace `ns` written as `object`, as no member of
    /// it, when all its members are known.
    fn absent(&mut self, expr: ExprId, object: ExprId, ns: &Namespace, name: Symbol) {
        let cx = self.env().cx;
        let Some((_, path)) = self.hir().path(object) else { return };
        if !Declared::complete(cx, ns) {
            return;
        }
        let msg =
            Msg::new().path(path.iter().map(|s| s.as_str())).text(" has no member ").code(name.as_str());
        let mut d = Diagnostic::error(errors::NAMES_UNRESOLVED, msg).at(self.hir().exprs[expr].origin, None);
        let exports = cx.get::<Exports>(ns);
        if let Some(near) = closest(name.as_str(), exports.iter().map(|s| s.as_str())) {
            d = d.child(ChildKind::Help, Msg::did_you_mean(near));
        }
        self.diag.error(d);
    }
}
