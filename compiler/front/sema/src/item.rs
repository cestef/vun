//! Where a namespace's members are declared (spec 01, section 8): in its file, through what it
//! spreads (selected names, or every export of a whole spread), or as a child namespace in its
//! directory.

use overt_hir::db::{Exports, FileRef, Namespace, NamespaceAt, Package, Parsed};
use overt_hir::tree::DeclId;
use overt_intern::Symbol;
use overt_query::{Cx, Cycle, Query};

/// A declaration, by its file and its place in the file's HIR.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DeclRef {
    pub file: FileRef,
    pub decl: DeclId,
}

/// What a member name leads to.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Site {
    Decl(DeclRef),
    /// A namespace, such as a file or directory below another.
    Namespace(Namespace),
}

/// Where the member `name` of a namespace is declared; `None` when it has no such member, or
/// only through spreads that loop.
pub struct Declared;

impl Query for Declared {
    const NAME: &'static str = "declared";
    type Key = (Namespace, Symbol);
    type Value = Option<Site>;

    fn compute(cx: &Cx<'_>, (ns, name): &Self::Key) -> Self::Value {
        let package = cx.get::<Package>(&ns.package);
        let file =
            package.file_of(&ns.path).map(|path| FileRef { package: ns.package, path: path.to_owned() });
        if let Some((file, parsed)) = file.and_then(|f| Some((f.clone(), cx.get::<Parsed>(&f)?))) {
            let hir = &parsed.hir;
            let root = hir.root?;
            if let Some(decl) = hir.declaring(root, *name) {
                return Some(Site::Decl(DeclRef { file, decl }));
            }
            for s in hir.spreads(root) {
                let selects =
                    s.select.as_ref().map(|names| names.iter().any(|&b| hir.binders[b].name == *name));
                let Some((_, path)) = hir.path(s.path) else { continue };
                let Ok(target) = cx.get::<NamespaceAt>(&(ns.package, path)) else { continue };
                let brings = selects.unwrap_or_else(|| cx.get::<Exports>(&target).contains(name));
                if brings {
                    return cx.get::<Self>(&(target, *name));
                }
            }
        }
        package.children(&ns.path).contains(name).then(|| {
            let path = ns.path.iter().copied().chain([*name]).collect();
            Site::Namespace(Namespace { package: ns.package, path })
        })
    }

    fn cycle(_: &Cx<'_>, _: &Self::Key, _: &Cycle) -> Self::Value {
        None
    }
}
