//! Impls and witnesses (spec 02, section 3). `#[impl(I(T))]` on a declaration registers its value,
//! the witness, for the interface `I` and the type `T`; the members of the witness, then the
//! defaults of `I(T)`, join `T`'s method lookup. `T` may be a type constructor, such as `lang.Int`
//! or a function returning a type: the impl then covers every type the constructor makes, and its
//! value is a function of the constructor's arguments giving the witness of each (decision 76).
//! `T` may also be an interface constructor, such as `lang.Coroutine`: the impl covers every type
//! implementing an interface it makes, and its value takes that type, then the interface's
//! arguments (decision 92). `#[impl(T)]`, with no interface, gives `T` the declarations of its
//! value as methods, found by the same lookup (decision 102).

mod lookup;
mod requirement;

use std::sync::Arc;

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
    /// `T` only.
    pub contract: Option<Contract>,
    /// `T`: a type, or a type constructor for every type it makes.
    pub target: Value,
}

/// The interface an impl implements.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Contract {
    /// `I`: its function from a type to the requirement struct.
    pub interface: Func,
    /// `I(T)` as the attribute wrote it, whose declarations are the requirements.
    pub requirements: Ty,
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
    /// declared interface, or, for the interface calls go through (the table `calls`), the
    /// parameters and result of a function or closure.
    #[must_use]
    pub fn implemented(&self, ty: Ty, constructor: &Value) -> Option<Vec<Value>> {
        if let Some(args) = ty.implements(constructor) {
            return Some(args);
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
                    if let Some((contract, target)) = env.impl_attr(attr) {
                        let decl = DeclRef::new(file.clone(), hir, *decl);
                        let attr = hir.exprs[attr].origin;
                        impls.push(ImplDecl { decl, attr, contract, target });
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
    /// The contract and the target of `#[impl(I(T))]` or `#[impl(T)]` at `attr`; `None` for
    /// any other attribute. The attribute is the primitive `lang.impl`; an argument that is an
    /// interface applied to a target gives the struct its function built, which records both.
    fn impl_attr(&self, attr: ExprId) -> Option<(Option<Contract>, Value)> {
        let hir = &self.parsed.hir;
        let ExprKind::Call { callee, args } = &hir.exprs[attr].kind else { return None };
        let Ok(Value::Primitive(Primitive::Lang(Lang::r#impl))) = self.eval(*callee) else { return None };
        let [Entry::One(Arg { name: None, value })] = args.as_slice() else { return None };
        let applied = match &hir.exprs[*value].kind {
            ExprKind::Call { callee, .. } => matches!(self.eval(*callee), Ok(Value::Interface(_))),
            _ => false,
        };
        if !applied {
            return Some((None, self.eval(*value).ok()?));
        }
        let requirements = self.ty(*value).ok()?;
        let TyKind::Nominal(Nominal { instance: Some(Instance { f, args }), .. }) = &*requirements else {
            return None;
        };
        let [target] = args.as_slice() else { return None };
        Some((Some(Contract { interface: f.clone(), requirements }), target.clone()))
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
