//! Where a namespace's members are declared (spec 01, section 8): where it is written (its file, or
//! the `struct` or `enum` literal a declaration binds), as an enum's variant, through what it
//! spreads (selected names, or every export of a whole spread), or as a child namespace in its
//! directory.

use vun_codec::{Decode, Encode};
use vun_hir::db::{Exports, FileRef, Namespace, NamespaceAt, Package, Parsed, Written};
use vun_hir::tree::{DeclId, ExprId, Hir, PatKind};
use vun_intern::Symbol;
use vun_origin::OriginId;
use vun_query::{Cx, Cycle, Query};

/// A node of a file's HIR: `id` finds it in the file, and its origin identifies it in every
/// process (decision 59). Two nodes are equal when both agree, and hash by both, as every value
/// hashes by what it compares by: after an edit that moved a node its place differs, so a query
/// keyed by it is another key, and a value holding it another fingerprint, rather than one
/// computed with, or reading, the place the node had before. Nodes order by their place.
#[derive(Clone, Copy, Debug, Encode, Decode)]
pub struct Node<Id> {
    pub id: Id,
    pub origin: OriginId,
}

/// A declaration, by its file and its place in the file's HIR.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Encode, Decode)]
pub struct DeclRef {
    pub file: FileRef,
    pub decl: Node<DeclId>,
}

/// An expression, by its file and its place in the file's HIR. A function literal's is the
/// function value's identity.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Encode, Decode)]
pub struct ExprRef {
    pub file: FileRef,
    pub expr: Node<ExprId>,
}

impl DeclRef {
    /// Whether the declaration binds a `mut` name: a static, whose value is read at run time.
    #[must_use]
    pub fn mutable(&self, cx: &Cx<'_>) -> bool {
        let Some(parsed) = cx.get::<Parsed>(&self.file) else { return false };
        let hir = &parsed.hir;
        match hir.pats[hir.decls[self.decl.id].pat].kind {
            PatKind::Bind(b) => hir.binders[b].mutable,
            _ => false,
        }
    }

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

impl<Id: PartialEq> PartialEq for Node<Id> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.origin == other.origin
    }
}

impl<Id: Eq> Eq for Node<Id> {}

impl<Id: std::hash::Hash> std::hash::Hash for Node<Id> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.origin.hash(state);
    }
}

impl<Id: Ord> PartialOrd for Node<Id> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<Id: Ord> Ord for Node<Id> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

/// What a member name leads to.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Site {
    Decl(DeclRef),
    /// A variant of the enum `decl` binds, as a spread of the enum brings it.
    Variant {
        decl: DeclRef,
        name: Symbol,
    },
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
        let mut file_level = true;
        if let Some(written) = cx.get::<Written>(ns)
            && let Some(parsed) = cx.get::<Parsed>(&written.file)
        {
            let hir = &parsed.hir;
            file_level = written.is_file(hir);
            if let Some(decl) = hir.declaring(written.ns, *name) {
                return Some(Site::Decl(DeclRef::new(written.file.clone(), hir, decl)));
            }
            if let Some(decl) = written.enumerated.filter(|_| written.variants(hir).contains(name)) {
                return Some(Site::Variant {
                    decl: DeclRef::new(written.file.clone(), hir, decl),
                    name: *name,
                });
            }
            for s in hir.spreads(written.ns) {
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
        (file_level && package.children(&ns.path).contains(name)).then(|| {
            let path = ns.path.iter().copied().chain([*name]).collect();
            Site::Namespace(Namespace { package: ns.package, path })
        })
    }

    fn cycle(_: &Cx<'_>, _: &Self::Key, _: &Cycle) -> Self::Value {
        None
    }
}

impl Declared {
    /// Whether every member of `ns` is named as written, none computed (`$(e) := ...`): only
    /// then is a name [`Declared`] does not find no member at all.
    #[must_use]
    pub fn complete(cx: &Cx<'_>, ns: &Namespace) -> bool {
        let Some(written) = cx.get::<Written>(ns) else { return true };
        let Some(parsed) = cx.get::<Parsed>(&written.file) else { return true };
        let hir = &parsed.hir;
        hir.member_decls(written.ns)
            .into_iter()
            .all(|d| !matches!(hir.pats[hir.decls[d].pat].kind, PatKind::Computed(_)))
    }
}
