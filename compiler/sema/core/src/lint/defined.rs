//! The lints a package defines, the declarations its files mark `#[lint.define]`, and those it is
//! checked with.

use std::sync::Arc;

use vun_diag::DiagId;
use vun_hir::db::{FileRef, Package, PackageId};
use vun_hir::tree::{Entry, Member};
use vun_intern::Symbol;
use vun_items::lang;
use vun_primitives::Primitive;
use vun_primitives::lint::Lint;
use vun_query::{Cx, Query};
use vun_sema_ty::item::DeclRef;

use super::tables::field;
use crate::decl::DeclValue;
use crate::env::Env;

/// The declarations marked `#[lint.define]` among the members of the files of a package.
pub struct DefinedLints;

impl Query for DefinedLints {
    const NAME: &'static str = "defined_lints";
    type Key = PackageId;
    type Value = Arc<Vec<DeclRef>>;

    fn compute(cx: &Cx<'_>, package: &PackageId) -> Self::Value {
        let files = cx.get::<Package>(package).files.clone();
        let files = files.iter().map(|path| FileRef { package: *package, path: Symbol::intern(path) });
        let defined = files.filter_map(|file| Env::of(cx, &file)).flat_map(|env| {
            let hir = &env.parsed.hir;
            let Some(root) = hir.root else { return Vec::new() };
            let members = hir.namespaces[root].members.iter().filter_map(|m| match m {
                Entry::One(Member::Decl(d)) => Some(*d),
                _ => None,
            });
            members
                .filter(|&d| env.attribute(&hir.decls[d].attrs, Primitive::Lint(Lint::define)).is_some())
                .map(|d| DeclRef::new(env.file.clone(), hir, d))
                .collect()
        });
        let defined: Vec<DeclRef> = defined.collect();
        vun_trace::tracing::debug!(?package, lints = defined.len(), "defined");
        Arc::new(defined)
    }
}

/// The lints a package is checked with, its own and those of the packages it depends on: each
/// lint's id, and its `doc` if it has one.
pub struct CheckedLints;

impl Query for CheckedLints {
    const NAME: &'static str = "checked_lints";
    type Key = PackageId;
    type Value = Arc<Vec<(DiagId, Option<String>)>>;

    fn compute(cx: &Cx<'_>, package: &PackageId) -> Self::Value {
        let data = cx.get::<Package>(package);
        let deps = data.deps.iter().filter_map(|d| d.package.as_ref().ok().copied());
        let mut packages: Vec<PackageId> = std::iter::once(*package).chain(deps).collect();
        packages.sort();
        packages.dedup();
        let defined = packages.iter().flat_map(|p| cx.get::<DefinedLints>(p).as_ref().clone());
        let lints = defined.filter_map(|decl| {
            let id = super::Lint::id_of(&super::Lint::path(cx, &decl)?);
            let value = cx.get::<DeclValue>(&decl).ok();
            let doc = value.as_ref().and_then(|v| field(v, lang::lint_Lint::doc)?.text());
            Some((id, doc))
        });
        Arc::new(lints.collect())
    }
}
