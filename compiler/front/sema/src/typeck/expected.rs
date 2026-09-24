//! Forms typed by what their context expects (spec 01, section 2): `.name` is a member of the
//! expected type (a variant, a variant's constructor, or a declaration of its namespace, or for
//! a type provided from outside, of the namespace declaring it).

use overt_hir::tree::ExprId;
use overt_intern::Symbol;

use super::{Known, Stage, Typer};
use crate::item::Declared;
use crate::member::{Found, MemberType, Members};
use crate::ty::{Signature, TyKind, Value};

impl Typer<'_, '_, '_> {
    /// `.name` where the context expects a type.
    pub(super) fn dot(&mut self, expr: ExprId, name: Symbol) -> Option<Known> {
        let expected = self.expected(expr)?;
        let origin = self.hir().exprs[expr].origin;
        let compile = |ty| Some(Known { ty, stage: Stage::Compile });
        match &*expected {
            TyKind::Nominal(_) => match self.env().cx.get::<Members>(&expected)?.find(name) {
                Found::Variant(v) if v.payload.is_empty() => compile(expected),
                Found::Variant(v) => {
                    let params = v.payload.iter().map(|p| p.as_ref().ok().copied()).collect::<Option<_>>()?;
                    compile(Signature::new(params, expected).ty())
                }
                Found::Decl(decl) => {
                    self.check_visible(expr, name, &decl.visible);
                    self.env().cx.get::<MemberType>(&(expected, name)).ok()
                }
                Found::Absent | Found::Field(_) => {
                    self.no_member(origin, expected, name);
                    None
                }
                Found::Unknown => None,
            },
            TyKind::Primitive(p, _) => {
                let Ok(Value::Namespace(ns)) = self.env().item(p.namespace()) else { return None };
                let Some(site) = self.env().cx.get::<Declared>(&(ns, name)) else {
                    self.no_member(origin, expected, name);
                    return None;
                };
                self.site(site)
            }
            TyKind::Tuple(_) | TyKind::Closure(_) | TyKind::Frame(_) => None,
        }
    }
}
