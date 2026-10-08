//! Finding a type's witnesses, and a method among them: `T`'s namespace first, then `T`'s impls
//! (spec 01, section 4).

use std::sync::Arc;

use vun_codec::{Decode, Encode};
use vun_hir::db::FileRef;
use vun_hir::tree::ExprKind;
use vun_intern::Symbol;
use vun_items::Item;
use vun_query::{Cx, Cycle, Query};
use vun_sema_ty::call::{Func, Instance};
use vun_sema_ty::item::DeclRef;
use vun_sema_ty::{Ty, TyKind, Value};

use super::{Candidate, Conformance, Derive, Derives, ImplDecl, Impls, Source, Unmet};
use crate::decl::DeclValue;
use crate::env::Env;
use crate::eval::Call;
use crate::member::{Found, Members};
use crate::visible::Visible;

/// The witness of an impl for one type.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
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
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Method {
    pub owner: Ty,
    pub decl: DeclRef,
    pub visible: Visible,
    /// The witness it is found through.
    pub witness: Witness,
}

/// The candidates for the witnesses of a type that a file sees, by the impls declared in its
/// scope and in the package declaring the type.
pub struct Witnesses;

impl Query for Witnesses {
    const NAME: &'static str = "witnesses";
    type Key = (FileRef, Ty);
    type Value = Arc<[Candidate]>;

    fn compute(cx: &Cx<'_>, (file, ty): &Self::Key) -> Self::Value {
        let Some(env) = Env::of(cx, file) else { return Arc::from([]) };
        let home = match &**ty {
            TyKind::Nominal(nominal) => Some(&nominal.literal.file),
            _ => None,
        };
        let files = env.impl_files(home);
        let impls = files.iter().flat_map(|f| cx.get::<Impls>(f).to_vec());
        let mut candidates: Vec<Candidate> = impls
            .filter(|decl| Witness::covering(decl, *ty).is_some())
            .map(|decl| Candidate { ty: *ty, source: Source::Impl(decl) })
            .collect();
        // An interface a type has no impl of may derive one from the type's parts.
        let derives: Vec<Derive> = files.iter().flat_map(|f| cx.get::<Derives>(f).to_vec()).collect();
        for derive in derives {
            if candidates.iter().any(|c| c.implements(&derive.interface)) {
                continue;
            }
            // Operators and methods reach a value through its borrows, so a borrowed part has
            // the interface its referent has.
            let has = |mut part: Ty| {
                while let Some(referent) = part.borrowed() {
                    part = referent;
                }
                Self::tiers(cx, file, part)
                    .iter()
                    .flat_map(|tier| tier.iter())
                    .any(|c| c.implements(&derive.interface))
            };
            if Witness::derivable(cx, *ty, has) {
                candidates.push(Candidate { ty: *ty, source: Source::Derive(derive) });
            }
        }
        candidates.into()
    }

    fn cycle(_: &Cx<'_>, _: &Self::Key, _: &Cycle) -> Self::Value {
        Arc::from([])
    }
}

impl Witnesses {
    /// The candidates `ty` has, in tiers, the nearest first: its own, then, for an array, those
    /// of the slice of its items (spec 05, section 1). An array is read as its items, as a borrow
    /// of it is a borrow of them, so it has the impls of that slice; what it has of itself serves
    /// first, a method or an interface it declares shadowing the slice's.
    pub(crate) fn tiers(cx: &Cx<'_>, file: &FileRef, ty: Ty) -> Vec<Arc<[Candidate]>> {
        let items = ty.as_array().map(|(_, items)| TyKind::slice(items));
        std::iter::once(ty).chain(items).map(|ty| cx.get::<Witnesses>(&(file.clone(), ty))).collect()
    }
}

impl Witness {
    /// The arguments the value of `decl` takes for `ty`, if its target covers `ty`: those of the
    /// constructor that made `ty`, or `ty` and the arguments of the interface it implements.
    pub(super) fn covering(decl: &ImplDecl, ty: Ty) -> Option<Vec<Value>> {
        ty.made_by(&decl.target).or_else(|| {
            let args = ty.implements(&decl.target)?;
            Some(std::iter::once(Value::Type(ty)).chain(args).collect())
        })
    }

    /// The witness `decl` gives for `ty`, if it covers `ty`.
    pub(super) fn of(cx: &Cx<'_>, decl: ImplDecl, ty: Ty) -> Option<Self> {
        let args = Self::covering(&decl, ty)?;
        let value = cx.get::<DeclValue>(&decl.decl).ok()?;
        let (witness, requirements) = if let Value::Type(_) = decl.target {
            (value.as_type()?, decl.contract.as_ref().map(|c| c.requirements))
        } else {
            // A constructor's witness is made for the arguments; a type taking none, such as a
            // primitive `Cx: type = _`, has the witness itself.
            let witness = match value {
                Value::Type(witness) => witness,
                value => Env::of(cx, &decl.decl.file)?.apply(value, args).ok()?.as_type()?,
            };
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

    /// The witness the holed impl `decl` gives for `ty` and the interface its constructor makes
    /// with `args`, each `None` where the use leaves it to the impl, where they fit what it
    /// writes: its value takes the values of its `_` after what its target gives.
    fn holed(cx: &Cx<'_>, decl: &ImplDecl, ty: Ty, args: &[Option<Value>]) -> Option<Self> {
        let holed = decl.holed.as_ref()?;
        let mut taken = Self::covering(decl, ty)?;
        let mut made = Vec::new();
        for (written, given) in holed.args.iter().zip(args) {
            match (written, given) {
                (Some(written), Some(given)) if written != given => return None,
                (Some(written), _) => made.push(written.clone()),
                (None, Some(given)) => {
                    taken.push(given.clone());
                    made.push(given.clone());
                }
                (None, None) => return None,
            }
        }
        let env = Env::of(cx, &decl.decl.file)?;
        let value = cx.get::<DeclValue>(&decl.decl).ok()?;
        let witness = env.apply(value, taken).ok()?.as_type()?;
        let Value::Interface(interface) = env.apply(Value::Fn(holed.constructor.clone()), made).ok()? else {
            return None;
        };
        let requirements =
            cx.get::<Call>(&Instance { f: interface, args: vec![Value::Type(ty)] }).ok()?.as_type()?;
        Some(Self { decl: decl.clone(), ty: witness, requirements: Some(requirements) })
    }

    /// Whether it is made for a use, by a type constructor or for the values of `_`, rather than
    /// declared as it is: then its requirements are checked where it is used.
    #[must_use]
    pub fn made(&self) -> bool {
        self.decl.holed.is_some() || !matches!(self.decl.target, Value::Type(_))
    }

    /// The requirements of its interface it does not meet.
    #[must_use]
    pub fn unmet(&self, cx: &Cx<'_>) -> Arc<[Unmet]> {
        self.requirements
            .map(|requirements| cx.get::<Conformance>(&(self.ty, requirements)))
            .unwrap_or_default()
    }

    /// The member `name` of this witness, or else the default of its interface.
    fn method(&self, cx: &Cx<'_>, name: Symbol) -> Option<Method> {
        let declared = |owner: Ty| match cx.get::<Members>(&owner)?.find(name) {
            Found::Decl(d) => Some(Method {
                owner,
                decl: d.decl.clone(),
                visible: d.visible.clone(),
                witness: self.clone(),
            }),
            _ => None,
        };
        declared(self.ty).or_else(|| declared(self.requirements?).filter(|m| !m.decl.required(cx)))
    }
}

/// A declaration as a requirement of an interface.
pub trait Required {
    /// Whether this declaration's value is `_`: a requirement, in an interface.
    #[must_use]
    fn required(&self, cx: &Cx<'_>) -> bool;
}

impl Required for DeclRef {
    fn required(&self, cx: &Cx<'_>) -> bool {
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

    /// The struct `ty`'s impl of `interface` gives, through the impls this file sees.
    #[must_use]
    pub fn witness(&self, interface: &Func, ty: Ty) -> Option<Ty> {
        let tiers = Witnesses::tiers(self.cx, &self.file, ty);
        tiers
            .iter()
            .flat_map(|tier| tier.iter())
            .filter(|c| c.implements(interface))
            .find_map(|c| c.witness(self.cx))
            .map(|w| w.ty)
    }

    /// The function `function` of the interface `interface` for `ty`: the witness's, or the
    /// interface's default there. What an operator calls (the tables `operators` and
    /// `prefix_operators`).
    #[must_use]
    pub fn implemented_by(&self, ty: Ty, interface: Item, function: Symbol) -> Option<Method> {
        let Ok(Value::Interface(interface)) = self.item(interface) else { return None };
        let tiers = Witnesses::tiers(self.cx, &self.file, ty);
        tiers
            .iter()
            .flat_map(|tier| tier.iter())
            .filter(|c| c.implements(&interface))
            .filter_map(|c| c.witness(self.cx))
            .find_map(|w| w.method(self.cx, function))
    }

    /// The function `function` of an interface the constructor `constructor` makes with `args`,
    /// each `None` where any value fits, for `ty`: of an impl of such an interface, or of one of
    /// every interface the constructor makes with any value where it writes `_`, whose witness is
    /// made for the values `args` give there (spec 02, section 3).
    #[must_use]
    pub fn made_implemented_by(
        &self,
        ty: Ty,
        constructor: Item,
        args: &[Option<Value>],
        function: Symbol,
    ) -> Option<Method> {
        let Ok(Value::Fn(constructor)) = self.item(constructor) else { return None };
        let fits = |made: &[Value]| {
            made.len() == args.len() && made.iter().zip(args).all(|(m, a)| a.as_ref().is_none_or(|a| a == m))
        };
        let made = |c: &Candidate| c.made().is_some_and(|i| i.f == constructor && fits(&i.args));
        let tiers = Witnesses::tiers(self.cx, &self.file, ty);
        let concrete = tiers.iter().flat_map(|tier| tier.iter()).filter(|c| made(c));
        if let Some(method) =
            concrete.filter_map(|c| c.witness(self.cx)).find_map(|w| w.method(self.cx, function))
        {
            return Some(method);
        }
        let home = match &*ty {
            TyKind::Nominal(nominal) => Some(&nominal.literal.file),
            _ => None,
        };
        let impls = self.impl_files(home).into_iter().flat_map(|f| self.cx.get::<Impls>(&f).to_vec());
        impls
            .filter(|d| d.holed.as_ref().is_some_and(|h| h.constructor == constructor))
            .filter_map(|d| Witness::holed(self.cx, &d, ty, args))
            .find_map(|w| w.method(self.cx, function))
    }

    /// Every method `name` that `ty` has through the impls this file sees, in the nearest tier
    /// that has one: more than one is a collision.
    #[must_use]
    pub fn methods(&self, ty: Ty, name: Symbol) -> Vec<Method> {
        let tiers = Witnesses::tiers(self.cx, &self.file, ty);
        let mut methods = tiers
            .iter()
            .map(|tier| tier.iter().filter_map(|c| c.witness(self.cx)?.method(self.cx, name)).collect());
        methods.find(|found: &Vec<Method>| !found.is_empty()).unwrap_or_default()
    }
}
