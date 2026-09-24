//! Impls and witnesses (spec 02, section 3). `#[impl(I(T))]` on a declaration registers its value,
//! the witness, for the interface `I` and the type `T`; the members of the witness, then the
//! defaults of `I(T)`, join `T`'s method lookup. `T` may be a type constructor, such as `lang.Int`
//! or a function returning a type: the impl then covers every type the constructor makes, and its
//! value is a function of the constructor's arguments giving the witness of each (decision 76).

mod lookup;

use std::sync::Arc;

use overt_hir::db::{FileRef, Package, PackageId};
use overt_hir::tree::{Arg, Entry, ExprId, ExprKind, Member, each_entry};
use overt_origin::OriginId;
use overt_query::{Cx, Cycle, Query};

use crate::Primitive;
use crate::env::Env;
use crate::eval::{Func, Instance};
use crate::item::DeclRef;
use crate::primitives::lang::Lang;
use crate::ty::{Nominal, Ty, TyKind, Value};

pub use lookup::{Method, Witness, Witnesses};

/// A declaration marked `#[impl(I(T))]`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ImplDecl {
    pub decl: DeclRef,
    /// The attribute, where the impl is reported.
    pub attr: OriginId,
    /// `I`: its function from a type to the requirement struct.
    pub interface: Func,
    /// `T`: a type, or a type constructor for every type it makes.
    pub target: Value,
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
        self.interface == other.interface
            && (self.target == other.target || covers(self, other) || covers(other, self))
    }
}

impl TyKind {
    /// The arguments `constructor` made this type from: none when the constructor is the type
    /// itself, those of `lang.Int(r)` for `lang.Int`, those of the call for a function returning
    /// a type.
    #[must_use]
    pub fn made_by(&self, constructor: &Value) -> Option<Vec<Value>> {
        match (constructor, self) {
            (Value::Type(ty), _) => (**ty == *self).then(Vec::new),
            (Value::Primitive(p), Self::Primitive(q, args)) => (p == q).then(|| args.clone()),
            (Value::Fn(f), Self::Nominal(Nominal { instance: Some(Instance { f: g, args }), .. })) => {
                (f == g).then(|| args.clone())
            }
            _ => None,
        }
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
                    if let Some((interface, target, requirements)) = env.impl_attr(attr) {
                        let decl = DeclRef::new(file.clone(), hir, *decl);
                        let attr = hir.exprs[attr].origin;
                        impls.push(ImplDecl { decl, attr, interface, target, requirements });
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
    /// The interface, the target and the requirement struct of `#[impl(I(T))]` at `attr`;
    /// `None` for any other attribute. The attribute is the primitive `lang.impl`, and `I(T)`
    /// the struct its interface's function built for `T`, which records both.
    fn impl_attr(&self, attr: ExprId) -> Option<(Func, Value, Ty)> {
        let ExprKind::Call { callee, args } = &self.parsed.hir.exprs[attr].kind else { return None };
        let Ok(Value::Primitive(Primitive::Lang(Lang::r#impl))) = self.eval(*callee) else { return None };
        let [Entry::One(Arg { name: None, value })] = args.as_slice() else { return None };
        let requirements = self.ty(*value).ok()?;
        let TyKind::Nominal(Nominal { instance: Some(Instance { f, args }), .. }) = &*requirements else {
            return None;
        };
        let [target] = args.as_slice() else { return None };
        Some((f.clone(), target.clone(), requirements))
    }
}

/// The files whose impls a file sees: itself, the namespaces of its package, and those of every
/// package its package depends on, directly or not (spec 02, section 3: an impl lives in the
/// package of its type or of its interface, which the code using both depends on).
pub(crate) fn scope(cx: &Cx<'_>, file: &FileRef) -> Vec<FileRef> {
    let mut packages: Vec<PackageId> = vec![file.package];
    let mut i = 0;
    while let Some(&package) = packages.get(i) {
        for dep in &cx.get::<Package>(&package).deps {
            if let Ok(dep) = &dep.package
                && !packages.contains(dep)
            {
                packages.push(*dep);
            }
        }
        i += 1;
    }
    let mut files = vec![file.clone()];
    for package in packages {
        let data = cx.get::<Package>(&package);
        let namespaces = data.files.iter().filter(|path| data.namespace_of(path).is_some());
        let paths = namespaces.map(|path| FileRef { package, path: path.clone() });
        files.extend(paths.filter(|f| f != file));
    }
    files
}
