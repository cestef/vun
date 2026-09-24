//! Finding a type's witnesses, and a method among them: `T`'s namespace first, then `T`'s impls
//! (spec 01, section 4).

use std::sync::Arc;

use overt_hir::db::FileRef;
use overt_hir::tree::ExprKind;
use overt_intern::Symbol;
use overt_query::{Cx, Cycle, Query};

use super::{ImplDecl, Impls, scope};
use crate::decl::DeclValue;
use crate::env::Env;
use crate::eval::{Call, Instance};
use crate::item::DeclRef;
use crate::member::{Found, Members};
use crate::ty::{Ty, Value};
use crate::visible::Visible;

/// The witness of an impl for one type.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Witness {
    pub decl: ImplDecl,
    /// The struct the impl gives for the type.
    pub ty: Ty,
    /// `I(T)` for the type, whose declarations with a value are the defaults.
    pub requirements: Ty,
}

/// A method a type has through an impl: a declaration of the witness, or a default of the
/// interface, which `owner` is the struct of.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Method {
    pub owner: Ty,
    pub decl: DeclRef,
    pub visible: Visible,
}

/// The witnesses of a type that a file sees, by the impls declared in its scope.
pub struct Witnesses;

impl Query for Witnesses {
    const NAME: &'static str = "witnesses";
    type Key = (FileRef, Ty);
    type Value = Arc<[Witness]>;

    fn compute(cx: &Cx<'_>, (file, ty): &Self::Key) -> Self::Value {
        let impls = scope(cx, file).into_iter().flat_map(|f| cx.get::<Impls>(&f).to_vec());
        impls.filter_map(|decl| Witness::of(cx, decl, *ty)).collect()
    }

    fn cycle(_: &Cx<'_>, _: &Self::Key, _: &Cycle) -> Self::Value {
        Arc::from([])
    }
}

impl Witness {
    /// The witness `decl` gives for `ty`, if it covers `ty`.
    fn of(cx: &Cx<'_>, decl: ImplDecl, ty: Ty) -> Option<Self> {
        let args = ty.made_by(&decl.target)?;
        let value = cx.get::<DeclValue>(&decl.decl).ok()?;
        let (witness, requirements) = if let Value::Type(_) = decl.target {
            (value.as_type()?, decl.requirements)
        } else {
            let env = Env::of(cx, &decl.decl.file)?;
            let witness = env.apply(value, args).ok()?.as_type()?;
            let call = Instance { f: decl.interface.clone(), args: vec![Value::Type(ty)] };
            (witness, cx.get::<Call>(&call).ok()?.as_type()?)
        };
        Some(Self { decl, ty: witness, requirements })
    }

    /// The member `name` of this witness, or else the default of its interface.
    fn method(&self, cx: &Cx<'_>, name: Symbol) -> Option<Method> {
        let declared = |owner: Ty| match cx.get::<Members>(&owner)?.find(name) {
            Found::Decl(d) => Some(Method { owner, decl: d.decl.clone(), visible: d.visible.clone() }),
            _ => None,
        };
        declared(self.ty).or_else(|| declared(self.requirements).filter(|m| !m.decl.required(cx)))
    }
}

impl DeclRef {
    /// Whether this declaration's value is `_`: a requirement, in an interface.
    #[must_use]
    pub fn required(&self, cx: &Cx<'_>) -> bool {
        let Some(env) = Env::of(cx, &self.file) else { return false };
        let hir = &env.parsed.hir;
        hir.decls[self.decl.id].value.is_some_and(|v| matches!(hir.exprs[v].kind, ExprKind::Hole))
    }
}

impl Env<'_, '_> {
    /// The method `name` that `ty` has through the impls this file sees.
    #[must_use]
    pub fn method(&self, ty: Ty, name: Symbol) -> Option<Method> {
        let witnesses = self.cx.get::<Witnesses>(&(self.file.clone(), ty));
        witnesses.iter().find_map(|w| w.method(self.cx, name))
    }
}
