//! The lints a package defines: the declarations its files mark `#[lint.define]`.

use std::sync::Arc;

use vun_hir::db::{FileRef, Package, PackageId};
use vun_hir::tree::{Entry, Member};
use vun_intern::Symbol;
use vun_primitives::Primitive;
use vun_primitives::lint::Lint;
use vun_query::{Cx, Query};
use vun_sema_ty::item::DeclRef;

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
