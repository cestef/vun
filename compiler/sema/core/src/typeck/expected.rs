//! What the context of a form expects of it (spec 01, section 2): the type it says, passed on to
//! the parts, and the forms typed by it: `.name` is a member of the expected type (a variant, a
//! variant's constructor, or a declaration of its namespace, or for a type provided from outside,
//! of the namespace declaring it).

use vun_hir::tree::{BlockId, ExprId};
use vun_intern::Symbol;
use vun_sema_ty::item::Declared;
use vun_sema_ty::{Signature, Ty, TyKind, Value};

use super::{GaveUp, Known, Stage, Typer};
use crate::member::{Found, MemberType, Members};

impl Typer<'_, '_, '_> {
    /// `.name` where the context expects a type.
    pub(super) fn dot(&mut self, expr: ExprId, name: Symbol) -> Option<Known> {
        // Without a type, or where a name is expected, `.name` is the name itself.
        let bare = Self::bare_name();
        let Some(expected) = self.expected(expr).filter(|&t| t != bare.ty) else { return Some(bare) };
        let origin = self.hir().exprs[expr].origin;
        let compile = |ty| Some(Known { ty, stage: Stage::Compile });
        self.finding().contexts.insert(expr, expected);
        match &*expected {
            TyKind::Nominal(_) => match self.env().cx.get::<Members>(&expected)?.find(name) {
                Found::Variant(v) if v.payload.is_empty() => compile(expected),
                Found::Variant(v) => {
                    let params = v.payload.iter().map(|p| p.as_ref().ok().copied()).collect::<Option<_>>()?;
                    compile(Signature::new(params, expected).ty())
                }
                Found::Decl(decl) => {
                    self.check_visible(expr, name, &decl.visible);
                    self.known(expr, self.env().cx.get::<MemberType>(&(expected, name)))
                }
                Found::Spread(site) => self.declared_at(expr, name, site.clone()),
                Found::Absent | Found::Field(_) => {
                    self.no_member(origin, expected, name);
                    None
                }
                Found::Unknown => {
                    self.give_up(expr, GaveUp::Members(expected));
                    None
                }
            },
            TyKind::Primitive(p, _) => {
                let Ok(Value::Namespace(ns)) = self.env().item(p.namespace()) else { return None };
                let Some(site) = self.env().cx.get::<Declared>(&(ns, name)) else {
                    self.no_member(origin, expected, name);
                    return None;
                };
                self.known(expr, self.env().site_type(site))
            }
            TyKind::Tuple(_)
            | TyKind::Record(_)
            | TyKind::Closure(_)
            | TyKind::Frame(_)
            | TyKind::Param(_)
            | TyKind::Generic(_) => None,
        }
    }

    /// The type the context of `expr` expects, if it says.
    pub(super) fn expected(&self, expr: ExprId) -> Option<Ty> {
        self.expected.get(&expr)?.as_ref().ok().copied()
    }

    /// The type `part` of `expr` is expected to have, as `derive` finds it from the type
    /// expected of `expr`; when that is not known here, neither is the part's, for the same
    /// reason.
    pub(super) fn expected_of(
        &mut self,
        expr: ExprId,
        part: ExprId,
        derive: impl FnOnce(Ty) -> Option<Ty>,
    ) -> Option<Ty> {
        let expected = self.expected.get(&expr).cloned()?;
        self.expecting(part, expected).and_then(derive)
    }

    /// The type `part` is expected to have, as `expected` says; when it is not known here, why
    /// is recorded for `part`.
    pub(super) fn expecting(&mut self, part: ExprId, expected: Result<Ty, GaveUp>) -> Option<Ty> {
        expected.map_err(|why| self.expected.insert(part, Err(why))).ok()
    }

    /// The type the tail of `block`, the value of `expr`, is expected to have: that of `expr`.
    pub(super) fn tail_expected(&mut self, expr: ExprId, block: BlockId) -> Option<Ty> {
        match self.hir().blocks[block].tail {
            Some(tail) => self.expected_of(expr, tail, Some),
            None => self.expected(expr),
        }
    }
}
