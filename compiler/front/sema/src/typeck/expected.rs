//! Forms typed by what their context expects (spec 01, section 2): `.name` is a member of the
//! expected type (a variant, a variant's constructor, or a declaration of its namespace, or for
//! a type provided from outside, of the namespace declaring it), and a tuple's items take the
//! expected items' types.

use overt_hir::tree::{Entry, ExprId};
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
            TyKind::Nominal(_) => match self.env.cx.get::<Members>(&expected)?.find(name) {
                Found::Variant(v) if v.payload.is_empty() => compile(expected),
                Found::Variant(v) => {
                    let params = v.payload.iter().map(|p| p.as_ref().ok().copied()).collect::<Option<_>>()?;
                    compile(Signature { params, result: expected }.ty())
                }
                Found::Decl(decl) => {
                    self.check_visible(expr, name, &decl.visible);
                    self.env.cx.get::<MemberType>(&(expected, name)).ok()
                }
                Found::Absent | Found::Field(_) => {
                    self.no_member(origin, expected, name);
                    None
                }
                Found::Unknown => None,
            },
            TyKind::Primitive(p, _) => {
                let Ok(Value::Namespace(ns)) = self.env.item(p.namespace()) else { return None };
                let Some(site) = self.env.cx.get::<Declared>(&(ns, name)) else {
                    self.no_member(origin, expected, name);
                    return None;
                };
                self.site(site)
            }
            _ => None,
        }
    }

    /// `(a, b)`: a tuple of its items' types, each typed as and checked against the expected
    /// tuple's item.
    pub(super) fn tuple(&mut self, expr: ExprId, items: &[Entry<ExprId>]) -> Option<Known> {
        let expected = self.expected(expr).and_then(|t| match &*t {
            TyKind::Tuple(parts) if parts.len() == items.len() => Some(parts.clone()),
            _ => None,
        });
        let mut parts = Vec::new();
        let mut stage = Stage::Compile;
        for (i, item) in items.iter().enumerate() {
            let Entry::One(item) = item else { return None };
            let part = expected.as_ref().map(|p| p[i]);
            let known = self.typed_as(*item, part);
            if let Some(known) = known {
                stage = stage.max(known.stage);
            }
            // An item that does not fit is reported on its own, and the tuple has no type.
            if let (Some(known), Some(part)) = (known, part)
                && !part.accepts(&known.ty)
            {
                self.expect(self.hir().exprs[*item].origin, known, part, None);
                parts.push(None);
                continue;
            }
            parts.push(known.map(|k| k.ty));
        }
        let parts = parts.into_iter().collect::<Option<_>>()?;
        Some(Known { ty: TyKind::Tuple(parts).intern(), stage })
    }
}
