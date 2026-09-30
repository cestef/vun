//! Namespaces and their members (spec 01, section 8): the declarations of a namespace's file, or of
//! the `struct` or `enum` literal a declaration of it binds, the public members of what it spreads
//! whole, an enum's variants, and what its directory holds.

use std::sync::Arc;

use vun_codec::{Decode, Encode};
use vun_diag::{Diagnostic, Msg, errors};
use vun_intern::Symbol;
use vun_query::{Cx, Cycle, Query};
use vun_syntax::visibility::Visibility;

use super::input::{FileRef, Package, PackageId, Unloaded};
use super::layout::Namespace;
use super::parsed::Parsed;
use crate::tree::{DeclId, ExprKind, Hir, Name, NsId, Spread};

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
        let note = Msg::new()
            .text("a namespace is a file or a directory of a package, named from the package or a dependency its ")
            .code("build.vun")
            .text(" declares");
        Diagnostic::error(errors::PACKAGE_MISSING_NAMESPACE, msg).note(note)
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
        let ns = Namespace { package: root, path: rest.to_vec() };
        let held = cx.get::<Package>(&root).holds(rest) || cx.get::<Written>(&ns).is_some();
        held.then_some(ns).ok_or(Unfound::Absent)
    }
}

/// Where a namespace is written: the file holding it, and the namespace of the file's tree its
/// path leads to: the file's own, or past it, that of the `struct` or `enum` literal each name
/// declares in the one before (`lang.arch.thumb.Cond`); for an enum literal, the declaration
/// binding it, whose variants are members too. `None` for a namespace no file holds, such as a
/// directory alone.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Written {
    pub file: FileRef,
    pub ns: NsId,
    pub enumerated: Option<DeclId>,
}

impl Query for Written {
    const NAME: &'static str = "written";
    type Key = Namespace;
    type Value = Option<Written>;

    fn compute(cx: &Cx<'_>, ns: &Namespace) -> Self::Value {
        let package = cx.get::<Package>(&ns.package);
        let (at, path) =
            (0..=ns.path.len()).rev().find_map(|n| Some((n, package.file_of(&ns.path[..n])?)))?;
        let file = FileRef { package: ns.package, path: Symbol::intern(path) };
        let parsed = cx.get::<Parsed>(&file)?;
        let hir = &parsed.hir;
        let mut written = Written { file, ns: hir.root?, enumerated: None };
        for &name in &ns.path[at..] {
            let decl = hir.declaring(written.ns, name)?;
            written = match hir.exprs[hir.decls[decl].value?].kind {
                ExprKind::Struct(inner) => Written { ns: inner, enumerated: None, ..written },
                ExprKind::Enum { ns: inner, .. } => Written { ns: inner, enumerated: Some(decl), ..written },
                _ => return None,
            };
        }
        Some(written)
    }
}

impl Written {
    /// Whether it is its file's own namespace, whose directory holds members too.
    #[must_use]
    pub fn is_file(&self, hir: &Hir) -> bool {
        hir.root == Some(self.ns)
    }

    /// The variants of the enum literal it is, written with plain names.
    #[must_use]
    pub fn variants(&self, hir: &Hir) -> Vec<Symbol> {
        let variants = self.enumerated.map(|d| hir.variants_of(d)).unwrap_or_default();
        variants
            .iter()
            .filter_map(|v| if let Name::Plain(name) = v.name { Some(name) } else { None })
            .collect()
    }
}

/// The names of a namespace: declared where it is written, exported by what it spreads whole, an
/// enum's variants, and held in its directory. A namespace reached again through its own spreads
/// contributes nothing more.
pub struct Members;

impl Query for Members {
    const NAME: &'static str = "namespace_members";
    type Key = Namespace;
    type Value = Arc<[Symbol]>;

    fn compute(cx: &Cx<'_>, ns: &Namespace) -> Self::Value {
        let package = cx.get::<Package>(&ns.package);
        let mut names = Vec::new();
        let mut file_level = true;
        if let Some(written) = cx.get::<Written>(ns)
            && let Some(parsed) = cx.get::<Parsed>(&written.file)
        {
            let hir = &parsed.hir;
            file_level = written.is_file(hir);
            names.extend(hir.members(written.ns).into_iter().map(|b| hir.binders[b].name));
            names.extend(written.variants(hir));
            // The names a spread selects are members as written, among the others.
            let whole = hir.spreads(written.ns).into_iter().filter(|s| s.select.is_none());
            for s in whole {
                if let Some(target) = s.target(cx, hir, ns.package) {
                    names.extend(s.names(cx, hir, &target).iter().copied());
                }
            }
        }
        if file_level {
            names.extend(package.children(&ns.path));
        }
        names.into()
    }

    fn cycle(_: &Cx<'_>, _: &Namespace, _: &Cycle) -> Self::Value {
        Arc::default()
    }
}

impl Spread {
    /// The namespace it names, read from the files of `package` alone (spec 01, section 8);
    /// `None` when it names none, which the name checks of its file report.
    #[must_use]
    pub fn target(&self, cx: &Cx<'_>, hir: &Hir, package: PackageId) -> Option<Namespace> {
        let (_, path) = hir.path(self.path)?;
        cx.get::<NamespaceAt>(&(package, path)).ok()
    }

    /// The names it brings from `target`, the namespace it names: those it selects, or every
    /// public one.
    #[must_use]
    pub fn names(&self, cx: &Cx<'_>, hir: &Hir, target: &Namespace) -> Arc<[Symbol]> {
        match &self.select {
            Some(select) => select.iter().map(|&b| hir.binders[b].name).collect(),
            None => cx.get::<Exports>(target),
        }
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
