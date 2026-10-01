//! Impls and witnesses (spec 02, section 3). `#[impl(I(T))]` on a declaration registers its value,
//! the witness, for the interface `I` and the type `T`; the members of the witness, then the
//! defaults of `I(T)`, join `T`'s method lookup. `T` may be a type constructor, such as `lang.Int`
//! or a function returning a type: the impl then covers every type the constructor makes, and its
//! value is a function of the constructor's arguments giving the witness of each (decision 76).
//! `T` may also be an interface constructor, such as `lang.Coroutine`: the impl covers every type
//! implementing an interface it makes, and its value takes that type, then the interface's
//! arguments (decision 92). `#[impl(T)]`, with no interface, gives `T` the declarations of its
//! value as methods, found by the same lookup (decision 102).

mod conformance;
mod derive;
mod lookup;
mod requirement;

use std::sync::Arc;

pub use conformance::{Conformance, Shortfall, Unmet};
pub use derive::{Derive, Derives};
pub use lookup::{Method, Required, Witness, Witnesses};
pub use requirement::{Implemented, Implementing, Requirement};
use vun_codec::{Decode, Encode};
use vun_desugar::CALLS;
use vun_hir::db::{FileRef, Package, PackageId};
use vun_hir::tree::{Arg, Entry, ExprId, ExprKind, Member, each_entry};
use vun_intern::Symbol;
use vun_origin::OriginId;
use vun_primitives::Primitive;
use vun_primitives::lang::Lang;
use vun_query::{Cx, Cycle, Query};
use vun_sema_ty::call::{Func, Instance};
use vun_sema_ty::item::DeclRef;
use vun_sema_ty::{Nominal, Ty, TyKind, Value};

use crate::env::Env;

/// A declaration marked `#[impl(I(T))]`, or `#[impl(T)]` for methods without an interface.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct ImplDecl {
    pub decl: DeclRef,
    /// The attribute, where the impl is reported.
    pub attr: OriginId,
    /// The interface implemented; `None` for `#[impl(T)]`, whose declarations are methods of
    /// `T` only, and for an impl of every interface a constructor makes (`holed`).
    pub contract: Option<Contract>,
    /// The interfaces implemented where the contract writes `_` among its arguments.
    pub holed: Option<Holed>,
    /// `T`: a type, or a type constructor for every type it makes.
    pub target: Value,
}

/// What an impl attribute implements, and for what.
struct ImplAttr {
    contract: Option<Contract>,
    holed: Option<Holed>,
    target: Value,
}

/// The interfaces an impl implements when its contract writes `_` among the arguments of the
/// interface's constructor: every one the constructor makes with any value there (spec 02,
/// section 3). The witness is made for the values a use gives the `_`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Holed {
    pub constructor: Func,
    /// The arguments as written, `None` for each `_`.
    pub args: Vec<Option<Value>>,
}

/// The interface an impl implements.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Contract {
    /// `I`: its function from a type to the requirement struct.
    pub interface: Func,
    /// `I(T)` as the attribute wrote it, whose declarations are the requirements.
    pub requirements: Ty,
}

impl Contract {
    /// The call making its interface: `lang.ops.Call((), ())` for `#[impl(lang.ops.Call((),
    /// ())(Case))]`.
    #[must_use]
    pub fn made(&self) -> Option<Instance> {
        self.interface.within.as_deref()?.call()
    }
}

impl ImplDecl {
    /// Whether this impl and `other` register a witness for a common pair `(I, T)`.
    #[must_use]
    pub fn overlaps(&self, other: &Self) -> bool {
        let covers = |a: &Self, b: &Self| match &b.target {
            Value::Type(ty) => ty.made_by(&a.target).is_some(),
            _ => false,
        };
        let same = match (&self.contract, &other.contract) {
            (Some(a), Some(b)) => a.interface == b.interface,
            _ => false,
        };
        same && (self.target == other.target || covers(self, other) || covers(other, self))
    }
}

impl Env<'_, '_> {
    /// The arguments of the interface `constructor` makes that `ty` implements: a frame's
    /// declared interface, an impl's for `ty`, or, for the interface calls go through (the table
    /// `calls`), the parameters and result of a function or closure.
    #[must_use]
    pub fn implemented(&self, ty: Ty, constructor: &Value) -> Option<Vec<Value>> {
        if let Some(args) = ty.implements(constructor) {
            return Some(args);
        }
        let tiers = Witnesses::tiers(self.cx, &self.file, ty);
        let made = tiers.iter().flat_map(|tier| tier.iter()).filter_map(|w| w.decl.contract.as_ref()?.made());
        if let Some(made) = made.into_iter().find(|made| Value::Fn(made.f.clone()) == *constructor) {
            return Some(made.args);
        }
        let interface = CALLS.first()?.parent()?;
        if self.item(interface).ok()? != *constructor {
            return None;
        }
        let signature = ty.callable()?;
        Some(vec![Value::Type(TyKind::Tuple(signature.params).intern()), Value::Type(signature.result)])
    }
}

/// The impls declared in a file, in source order: in any namespace of it, including `struct`
/// bodies.
pub struct Impls;

impl Query for Impls {
    const NAME: &'static str = "impls";
    type Key = FileRef;
    type Value = Arc<[ImplDecl]>;

    fn compute(cx: &Cx<'_>, file: &FileRef) -> Self::Value {
        let Some(env) = Env::of(cx, file) else { return Arc::from([]) };
        let hir = &env.parsed.hir;
        let mut impls = Vec::new();
        for (_, ns) in hir.namespaces.iter() {
            each_entry(&ns.members, &mut |member| {
                let Member::Decl(decl) = member else { return };
                for &attr in &hir.decls[*decl].attrs {
                    if let Some(ImplAttr { contract, holed, target }) = env.impl_attr(attr) {
                        let decl = DeclRef::new(file.clone(), hir, *decl);
                        let attr = hir.exprs[attr].origin;
                        impls.push(ImplDecl { decl, attr, contract, holed, target });
                    }
                }
            });
        }
        impls.into()
    }

    fn cycle(_: &Cx<'_>, _: &FileRef, _: &Cycle) -> Self::Value {
        Arc::from([])
    }
}

impl Env<'_, '_> {
    /// What `#[impl(I(T))]`, `#[impl(C(a, _)(T))]` or `#[impl(T)]` at `attr` implements, and for
    /// what; `None` for any other attribute. The attribute is the primitive `lang.impl`; an
    /// argument that is an interface applied to a target gives the struct its function built,
    /// which records both.
    fn impl_attr(&self, attr: ExprId) -> Option<ImplAttr> {
        let hir = &self.parsed.hir;
        let ExprKind::Call { callee, args } = &hir.exprs[attr].kind else { return None };
        let Ok(Value::Primitive(Primitive::Lang(Lang::r#impl))) = self.eval(*callee) else { return None };
        let [Entry::One(Arg { name: None, value })] = args.as_slice() else { return None };
        let implemented = |contract, holed, target| Some(ImplAttr { contract, holed, target });
        let ExprKind::Call { callee: interface, args: target } = &hir.exprs[*value].kind else {
            return implemented(None, None, self.eval(*value).ok()?);
        };
        if let Some(holed) = self.holed(*interface) {
            let [Entry::One(Arg { name: None, value: target })] = target.as_slice() else { return None };
            return implemented(None, Some(holed), self.eval(*target).ok()?);
        }
        if !matches!(self.eval(*interface), Ok(Value::Interface(_))) {
            return implemented(None, None, self.eval(*value).ok()?);
        }
        let requirements = self.ty(*value).ok()?;
        let TyKind::Nominal(Nominal { instance: Some(made), .. }) = &*requirements else { return None };
        let Instance { f, args } = made.call()?;
        let [target] = args.as_slice() else { return None };
        implemented(Some(Contract { interface: f, requirements }), None, target.clone())
    }

    /// The interfaces `C(a, _, ...)` at `expr` stands for, where it writes `_` among the
    /// arguments of an interface constructor.
    fn holed(&self, expr: ExprId) -> Option<Holed> {
        let hir = &self.parsed.hir;
        let ExprKind::Call { callee, args } = &hir.exprs[expr].kind else { return None };
        let hole = |a: &Entry<Arg>| matches!(a, Entry::One(Arg { value, .. }) if matches!(hir.exprs[*value].kind, ExprKind::Hole));
        if !args.iter().any(hole) {
            return None;
        }
        let Ok(Value::Fn(constructor)) = self.eval(*callee) else { return None };
        let args = args
            .iter()
            .map(|a| match a {
                Entry::One(Arg { name: None, value }) if matches!(hir.exprs[*value].kind, ExprKind::Hole) => {
                    Some(None)
                }
                Entry::One(Arg { name: None, value }) => Some(Some(self.eval(*value).ok()?)),
                _ => None,
            })
            .collect::<Option<_>>()?;
        Some(Holed { constructor, args })
    }
}

impl Env<'_, '_> {
    /// The files whose impls this file sees: itself, the namespaces of its package, and those of every
    /// package its package depends on, directly or not (spec 02, section 3: an impl lives in the
    /// package of its type or of its interface, which the code using both depends on); and those
    /// of `home`, the file a type is declared in, and its package, where a generic body
    /// instantiated with the type finds the type's impls.
    #[must_use]
    pub fn impl_files(&self, home: Option<&FileRef>) -> Vec<FileRef> {
        let mut packages: Vec<PackageId> = vec![self.file.package];
        let mut i = 0;
        while let Some(&package) = packages.get(i) {
            for dep in &self.cx.get::<Package>(&package).deps {
                if let Ok(dep) = &dep.package
                    && !packages.contains(dep)
                {
                    packages.push(*dep);
                }
            }
            i += 1;
        }
        packages.extend(home.map(|f| f.package).filter(|home| !packages.contains(home)));
        let mut files = vec![self.file.clone()];
        files.extend(home.filter(|&home| *home != self.file).cloned());
        for package in packages {
            let data = self.cx.get::<Package>(&package);
            let namespaces = data.files.iter().filter(|path| data.namespace_of(path).is_some());
            let paths = namespaces.map(|path| FileRef { package, path: Symbol::intern(path) });
            files.extend(paths.filter(|f| *f != self.file && Some(f) != home));
        }
        files
    }
}
