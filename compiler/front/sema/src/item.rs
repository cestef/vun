//! Where a namespace's members are declared (spec 01, section 8): in its file, through what it
//! spreads (selected names, or every export of a whole spread), or as a child namespace in its
//! directory.

use overt_hir::db::{Exports, FileRef, Namespace, NamespaceAt, Package, Parsed};
use overt_hir::tree::{DeclId, ExprId, Hir};
use overt_intern::Symbol;
use overt_origin::OriginId;
use overt_query::{Cx, Cycle, Query};

/// A node of a file's HIR: `id` finds it in the file, and its origin identifies it, so that it
/// hashes the same in every process and after edits elsewhere in the file (decision 59).
#[derive(Clone, Copy, Debug)]
pub struct Node<Id> {
    pub id: Id,
    pub origin: OriginId,
}

/// A declaration, by its file and its place in the file's HIR.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DeclRef {
    pub file: FileRef,
    pub decl: Node<DeclId>,
}

/// An expression, by its file and its place in the file's HIR. A function literal's is the
/// function value's identity.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ExprRef {
    pub file: FileRef,
    pub expr: Node<ExprId>,
}

impl DeclRef {
    #[must_use]
    pub fn new(file: FileRef, hir: &Hir, decl: DeclId) -> Self {
        Self { file, decl: Node { id: decl, origin: hir.decls[decl].origin } }
    }
}

impl ExprRef {
    #[must_use]
    pub fn new(file: FileRef, hir: &Hir, expr: ExprId) -> Self {
        Self { file, expr: Node { id: expr, origin: hir.exprs[expr].origin } }
    }
}

impl<Id> PartialEq for Node<Id> {
    fn eq(&self, other: &Self) -> bool {
        self.origin == other.origin
    }
}

impl<Id> Eq for Node<Id> {}

impl<Id> std::hash::Hash for Node<Id> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.origin.hash(state);
    }
}

impl<Id> PartialOrd for Node<Id> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<Id> Ord for Node<Id> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.origin.cmp(&other.origin)
    }
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
                return Some(Site::Decl(DeclRef::new(file, hir, decl)));
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
