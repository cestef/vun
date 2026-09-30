//! Names and members: a local, a declaration, a namespace, a member of a type's namespace or of
//! a value.

use vun_hir::resolve::Res;
use vun_hir::tree::{ExprId, ExprKind};
use vun_intern::Symbol;
use vun_sema_ty::item::{Declared, Site};
use vun_sema_ty::{Signature, Ty, TyKind, Value};

use super::{GaveUp, Known, Stage, Typer};
use crate::env::Env;
use crate::member::{Found, MemberType, Members};

impl Typer<'_, '_, '_> {
    /// A local by the type it was bound with; any other name by its declaration's type.
    pub(super) fn name(&mut self, expr: ExprId) -> Option<Known> {
        self.mutable_static(expr);
        if let Some(Res::Binder(b)) = self.env().parsed.resolution.names[expr] {
            if let Some(known) = self.locals.get(&b) {
                return Some(Known { ty: self.narrowed.get(&b).copied().unwrap_or(known.ty), ..*known });
            }
            // A binder the environment binds, such as a `$for` binder: its value's type.
            if let Some(ty) = self.env().bound(b).and_then(|v| self.env().value_type(v).ok()) {
                return Some(Known { ty, stage: Stage::Compile });
            }
        }
        let declared = self.env().resolve(expr).and_then(|site| self.env().site_type(site));
        // A declaration whose type is not known apart from its value, such as a requirement an
        // impl provides, has its value's type.
        let valued = declared.or_else(|why| {
            let ty =
                self.evaluating().eval(expr).ok().and_then(|v| self.env().value_type(&v).ok()).ok_or(why)?;
            Ok(Known { ty, stage: Stage::Compile })
        });
        self.known(expr, valued)
    }

    /// A member of a namespace, of a type's namespace, or of a value: a field, or a declaration of
    /// its type, called as a method.
    pub(super) fn member(&mut self, expr: ExprId, object: ExprId, name: Symbol) -> Option<Known> {
        let known = self.typed(object);
        // A path, or any other expression whose value is a type, such as `Pair(u8)`, names a
        // namespace or a type; nothing else is evaluated here.
        let path = matches!(self.hir().exprs[object].kind, ExprKind::Name(_) | ExprKind::Member { .. });
        if path || known.is_some_and(|k| k.ty == TyKind::of_types()) {
            match self.evaluating().eval(object) {
                // A member missing from a namespace is reported by the check of the file, which
                // reads every path, typed or not.
                Ok(Value::Namespace(ns)) => {
                    let site = self.env().cx.get::<Declared>(&(ns.clone(), name))?;
                    return self.declared_at(expr, name, site);
                }
                Ok(Value::Type(ty)) => {
                    let members = self.env().cx.get::<Members>(&ty);
                    return match members.as_deref().map_or(Found::Unknown, |m| m.find(name)) {
                        Found::Decl(decl) => {
                            self.check_visible(expr, name, &decl.visible);
                            self.known(expr, self.env().cx.get::<MemberType>(&(ty, name)))
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
                        Found::Spread(site) => self.declared_at(expr, name, site.clone()),
                        Found::Field(_) => {
                            self.field_of_type(self.hir().exprs[expr].origin, ty, name);
                            None
                        }
                        found @ (Found::Absent | Found::Unknown) => {
                            let Some(owner) = self.method(expr, ty, name) else {
                                if !matches!(found, Found::Unknown) || !Self::declares(ty) {
                                    self.no_member(self.hir().exprs[expr].origin, ty, name);
                                }
                                self.give_up(expr, GaveUp::Members(ty));
                                return None;
                            };
                            self.known(expr, self.env().cx.get::<MemberType>(&(owner, name)))
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
        let unknown = if Self::declares(known.ty) { Found::Unknown } else { Found::Absent };
        match members.as_deref().map_or(unknown, |m| m.find(name)) {
            Found::Field(field) => {
                self.check_visible(expr, name, &field.visible);
                let ty = field.ty.clone();
                self.known(expr, ty.map(|ty| Known { ty, stage: known.stage }))
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
                self.finding().methods.insert(expr, (known.ty, name));
                self.known(expr, self.env().cx.get::<MemberType>(&(known.ty, name)))
            }
            Found::Spread(site) => {
                self.finding().methods.insert(expr, (known.ty, name));
                self.declared_at(expr, name, site.clone())
            }
            // A member missing from the type and its impls; a variant is reached through its
            // type, never through a value, so a value's lookup goes on to the impls.
            found @ (Found::Variant(_) | Found::Absent | Found::Unknown) => {
                let Some(owner) = self.method(expr, known.ty, name) else {
                    if !matches!(found, Found::Unknown) {
                        self.no_member(self.hir().exprs[expr].origin, known.ty, name);
                    }
                    self.give_up(expr, GaveUp::Members(known.ty));
                    return None;
                };
                self.finding().methods.insert(expr, (owner, name));
                self.known(expr, self.env().cx.get::<MemberType>(&(owner, name)))
            }
        }
    }

    /// The type of the member `name` the path `expr` reaches, declared at `site`, which a spread
    /// keeps visible as its declaration is (spec 01, section 8).
    pub(super) fn declared_at(&mut self, expr: ExprId, name: Symbol, site: Site) -> Option<Known> {
        if let Site::Decl(decl) = &site
            && let Some(visible) =
                Env::of(self.env().cx, &decl.file).map(|e| e.decl_visible(decl.decl.id, name))
        {
            self.check_visible(expr, name, &visible);
        }
        self.known(expr, self.env().site_type(site))
    }

    /// Whether members of `ty` may be declared where they are not known here: a nominal type, a
    /// type, an interface or a value declaring its own, or a parameter or frame, whose members
    /// its interface requires.
    fn declares(ty: Ty) -> bool {
        matches!(&*ty, TyKind::Nominal(_)) || ty.is_required() || ty.declares_members()
    }
}
