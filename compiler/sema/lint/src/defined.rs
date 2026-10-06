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
use vun_sema_ty::Value;
use vun_sema_ty::item::DeclRef;

use vun_sema::decl::DeclValue;
use vun_sema::env::Env;

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
/// lint's id and its `doc` if it has one, and the id of each group they name.
pub struct CheckedLints;

/// What checking a package is configured by: the lints it sees, and their groups.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Checked {
    /// Each lint's id, with its `doc` if it has one.
    pub lints: Vec<(DiagId, Option<String>)>,
    /// Each group's id, `std.lints.<group>`.
    pub groups: Vec<DiagId>,
}

impl Query for CheckedLints {
    const NAME: &'static str = "checked_lints";
    type Key = PackageId;
    type Value = Arc<Checked>;

    fn compute(cx: &Cx<'_>, package: &PackageId) -> Self::Value {
        let data = cx.get::<Package>(package);
        let deps = data.deps.iter().filter_map(|d| d.package.as_ref().ok().copied());
        let mut packages: Vec<PackageId> = std::iter::once(*package).chain(deps).collect();
        packages.sort();
        packages.dedup();
        let defined = packages.iter().flat_map(|p| cx.get::<DefinedLints>(p).as_ref().clone());
        let mut lints = Vec::new();
        let mut groups = Vec::new();
        for decl in defined {
            let Some(id) = super::Lint::path(cx, &decl).map(|path| super::Lint::id_of(&path)) else {
                continue;
            };
            let value = cx.get::<DeclValue>(&decl).ok();
            let doc = value.as_ref().and_then(|v| v.field(lang::lint_Lint::doc)?.text());
            if let Some(Value::Name(group)) = value.as_ref().and_then(|v| v.field(lang::lint_Lint::group)) {
                let group = super::Lint::group_id(*group);
                if !groups.contains(&group) {
                    groups.push(group);
                }
            }
            lints.push((id, doc));
        }
        Arc::new(Checked { lints, groups })
    }
}
