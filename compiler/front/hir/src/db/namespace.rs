//! Namespaces and their members (spec 01, section 8): the declarations of a namespace's file, the
//! public members of what it spreads whole, and what its directory holds.

use std::sync::Arc;

use vun_codec::{Decode, Encode};
use vun_diag::{Diagnostic, Msg, errors};
use vun_intern::Symbol;
use vun_query::{Cx, Cycle, Query};
use vun_syntax::visibility::Visibility;

use super::input::{FileRef, Package, PackageId, Unloaded};
use super::layout::Namespace;
use super::parsed::Parsed;

/// Why a path names no namespace.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Unfound {
    /// Its first name is not the package or a dependency.
    Root,
    /// The dependency it starts at cannot be loaded, and why.
    Dep(Unloaded),
    /// Nothing in the package holds the path.
    Absent,
}

impl Unfound {
    /// That `path` names no namespace.
    #[must_use]
    pub fn not_namespace(path: &str) -> Diagnostic {
        let msg = Msg::new().code(path).text(" is not a namespace of this package or a dependency");
        Diagnostic::error(errors::PACKAGE_MISSING_NAMESPACE, msg)
    }
}

/// The namespace at a path as seen from a package: its first name is the package itself or a
/// dependency.
pub struct NamespaceAt;

impl Query for NamespaceAt {
    const NAME: &'static str = "namespace_at";
    type Key = (PackageId, Vec<Symbol>);
    type Value = Result<Namespace, Unfound>;

    fn compute(cx: &Cx<'_>, (package, path): &Self::Key) -> Self::Value {
        let (&first, rest) = path.split_first().ok_or(Unfound::Root)?;
        let data = cx.get::<Package>(package);
        let root = if data.name == Some(first) {
            *package
        } else {
            let dep = data.deps.iter().find(|d| d.name == first).ok_or(Unfound::Root)?;
            dep.package.clone().map_err(Unfound::Dep)?
        };
        let held = cx.get::<Package>(&root).holds(rest);
        held.then(|| Namespace { package: root, path: rest.to_vec() }).ok_or(Unfound::Absent)
    }
}

/// The names of a namespace: declared in its file, exported by what it spreads whole, and held
/// in its directory. A namespace reached again through its own spreads contributes nothing more.
pub struct Members;

impl Query for Members {
    const NAME: &'static str = "namespace_members";
    type Key = Namespace;
    type Value = Arc<[Symbol]>;

    fn compute(cx: &Cx<'_>, ns: &Namespace) -> Self::Value {
        let package = cx.get::<Package>(&ns.package);
        let mut names = Vec::new();
        let file =
            package.file_of(&ns.path).map(|path| FileRef { package: ns.package, path: Symbol::intern(path) });
        if let Some(parsed) = file.and_then(|f| cx.get::<Parsed>(&f)) {
            let hir = &parsed.hir;
            if let Some(root) = hir.root {
                names.extend(hir.members(root).into_iter().map(|b| hir.binders[b].name));
                let whole = hir.spreads(root).into_iter().filter(|s| s.select.is_none());
                for (_, path) in whole.filter_map(|s| hir.path(s.path)) {
                    if let Ok(target) = cx.get::<NamespaceAt>(&(ns.package, path)) {
                        names.extend(cx.get::<Exports>(&target).iter().copied());
                    }
                }
            }
        }
        names.extend(package.children(&ns.path));
        names.into()
    }

    fn cycle(_: &Cx<'_>, _: &Namespace, _: &Cycle) -> Self::Value {
        Arc::default()
    }
}

/// The public members of a namespace, which other namespaces see.
pub struct Exports;

impl Query for Exports {
    const NAME: &'static str = "exports";
    type Key = Namespace;
    type Value = Arc<[Symbol]>;

    fn compute(cx: &Cx<'_>, ns: &Namespace) -> Self::Value {
        let members = cx.get::<Members>(ns);
        members.iter().copied().filter(|n| Visibility::of(n.as_str()) == Visibility::Public).collect()
    }

    fn cycle(_: &Cx<'_>, _: &Namespace, _: &Cycle) -> Self::Value {
        Arc::default()
    }
}
