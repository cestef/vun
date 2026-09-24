//! Finding a type's witnesses, and a method among them: `T`'s namespace first, then `T`'s impls
//! (spec 01, section 4).

use std::sync::Arc;

use overt_hir::db::FileRef;
use overt_hir::tree::ExprKind;
use overt_intern::Symbol;
use overt_query::{Cx, Cycle, Query};

use super::{ImplDecl, Impls};
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
    /// `I(T)` for the type, whose declarations with a value are the defaults; `None` for an impl
    /// without an interface.
    pub requirements: Option<Ty>,
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
        let Some(env) = Env::of(cx, file) else { return Arc::from([]) };
        let impls = env.impl_files().into_iter().flat_map(|f| cx.get::<Impls>(&f).to_vec());
        impls.filter_map(|decl| Witness::of(cx, decl, *ty)).collect()
    }

    fn cycle(_: &Cx<'_>, _: &Self::Key, _: &Cycle) -> Self::Value {
        Arc::from([])
    }
}

impl Witness {
    /// The witness `decl` gives for `ty`, if it covers `ty`.
    fn of(cx: &Cx<'_>, decl: ImplDecl, ty: Ty) -> Option<Self> {
        let args = ty.made_by(&decl.target).or_else(|| {
            let args = ty.implements(&decl.target)?;
            Some(std::iter::once(Value::Type(ty)).chain(args).collect())
        })?;
        let value = cx.get::<DeclValue>(&decl.decl).ok()?;
        let (witness, requirements) = if let Value::Type(_) = decl.target {
            (value.as_type()?, decl.contract.as_ref().map(|c| c.requirements))
        } else {
            let env = Env::of(cx, &decl.decl.file)?;
            let witness = env.apply(value, args).ok()?.as_type()?;
            let requirements = match &decl.contract {
                Some(contract) => {
                    let call = Instance { f: contract.interface.clone(), args: vec![Value::Type(ty)] };
                    Some(cx.get::<Call>(&call).ok()?.as_type()?)
                }
                None => None,
            };
            (witness, requirements)
        };
        Some(Self { decl, ty: witness, requirements })
    }

    /// The member `name` of this witness, or else the default of its interface.
    fn method(&self, cx: &Cx<'_>, name: Symbol) -> Option<Method> {
        let declared = |owner: Ty| match cx.get::<Members>(&owner)?.find(name) {
            Found::Decl(d) => Some(Method { owner, decl: d.decl.clone(), visible: d.visible.clone() }),
            _ => None,
        };
        declared(self.ty).or_else(|| declared(self.requirements?).filter(|m| !m.decl.required(cx)))
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
        self.methods(ty, name).into_iter().next()
    }

    /// Every method `name` that `ty` has through the impls this file sees: more than one is a
    /// collision.
    #[must_use]
    pub fn methods(&self, ty: Ty, name: Symbol) -> Vec<Method> {
        let witnesses = self.cx.get::<Witnesses>(&(self.file.clone(), ty));
        witnesses.iter().filter_map(|w| w.method(self.cx, name)).collect()
    }
}
