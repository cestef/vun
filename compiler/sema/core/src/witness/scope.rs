//! The impls and derives a file sees (spec 02, section 3): its own, those of the namespaces of
//! its package and of every package it depends on, directly or not. The namespaces of a package
//! are read as one list per package, so that finding the witnesses of a type reads a handful of
//! memos and not one for each file of the toolchain.

use std::sync::Arc;

use vun_hir::db::{FileRef, Package, PackageId};
use vun_intern::Symbol;
use vun_query::{Cx, Cycle, Query};

use super::{Derive, Derives, ImplDecl, Impls};
use crate::env::Env;

/// Where the impls a file sees are declared: the files named, then every namespace of the
/// packages.
pub struct Scope {
    pub own: Vec<FileRef>,
    pub packages: Vec<PackageId>,
}

impl Scope {
    /// The files of `package` that are namespaces, in the order the package lists them.
    fn namespaces(cx: &Cx<'_>, package: PackageId) -> Vec<FileRef> {
        let data = cx.get::<Package>(&package);
        let namespaces = data.files.iter().filter(|path| data.namespace_of(path).is_some());
        namespaces.map(|path| FileRef { package, path: Symbol::intern(path) }).collect()
    }
}

/// The impls declared in the namespaces of a package, file by file.
pub struct PackageImpls;

impl Query for PackageImpls {
    const NAME: &'static str = "package_impls";
    type Key = PackageId;
    type Value = Arc<[ImplDecl]>;

    fn compute(cx: &Cx<'_>, package: &PackageId) -> Self::Value {
        Scope::namespaces(cx, *package).iter().flat_map(|f| cx.get::<Impls>(f).to_vec()).collect()
    }

    fn cycle(_: &Cx<'_>, _: &PackageId, _: &Cycle) -> Self::Value {
        Arc::from([])
    }
}

/// The derives declared in the namespaces of a package, file by file.
pub struct PackageDerives;

impl Query for PackageDerives {
    const NAME: &'static str = "package_derives";
    type Key = PackageId;
    type Value = Arc<[Derive]>;

    fn compute(cx: &Cx<'_>, package: &PackageId) -> Self::Value {
        Scope::namespaces(cx, *package).iter().flat_map(|f| cx.get::<Derives>(f).to_vec()).collect()
    }

    fn cycle(_: &Cx<'_>, _: &PackageId, _: &Cycle) -> Self::Value {
        Arc::from([])
    }
}

impl Env<'_, '_> {
    /// Where the impls this file sees are declared: itself, the namespaces of its package, and
    /// those of every package its package depends on, directly or not (spec 02, section 3: an
    /// impl lives in the package of its type or of its interface, which the code using both
    /// depends on); and those of `home`, the file a type is declared in, and its package, where a
    /// generic body instantiated with the type finds the type's impls.
    #[must_use]
    pub fn impl_scope(&self, home: Option<&FileRef>) -> Scope {
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
        let mut own = vec![self.file.clone()];
        own.extend(home.filter(|&home| *home != self.file).cloned());
        Scope { own, packages }
    }

    /// The files whose impls this file sees: those of `impl_scope`, each once.
    #[must_use]
    pub fn impl_files(&self, home: Option<&FileRef>) -> Vec<FileRef> {
        let Scope { mut own, packages } = self.impl_scope(home);
        let named = own.len();
        for package in packages {
            let rest = Scope::namespaces(self.cx, package).into_iter().filter(|f| !own[..named].contains(f));
            own.extend(rest.collect::<Vec<_>>());
        }
        own
    }

    /// The impls this file sees, those of the files it names first, then those of each package's
    /// namespaces.
    #[must_use]
    pub fn impls(&self, home: Option<&FileRef>) -> Vec<ImplDecl> {
        let Scope { own, packages } = self.impl_scope(home);
        let named = own.iter().flat_map(|f| self.cx.get::<Impls>(f).to_vec());
        let rest = packages.iter().flat_map(|p| self.cx.get::<PackageImpls>(p).to_vec());
        named.chain(rest.filter(|d| !own.contains(&d.decl.file))).collect()
    }

    /// The derives this file sees, in the order of [`Env::impls`].
    #[must_use]
    pub fn derives(&self, home: Option<&FileRef>) -> Vec<Derive> {
        let Scope { own, packages } = self.impl_scope(home);
        let named = own.iter().flat_map(|f| self.cx.get::<Derives>(f).to_vec());
        let rest = packages.iter().flat_map(|p| self.cx.get::<PackageDerives>(p).to_vec());
        named.chain(rest.filter(|d| !own.contains(&d.decl.file))).collect()
    }
}
