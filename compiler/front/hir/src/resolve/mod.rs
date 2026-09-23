//! Name resolution within a file, in one walk that also runs the checks needing no types
//! (spec 01, sections 3, 4 and 6).
//!
//! Namespaces (the file, `struct` bodies) declare all their members before anything in them is
//! resolved, so their order does not matter. Blocks declare in order. A function literal sees the
//! namespaces around it and the compile-time (`$`) binders of enclosing functions, but not their
//! run-time locals: functions do not capture, closures list what they capture. A name found
//! nowhere in the file is [`Res::Outer`], left to package resolution.

mod declare;
mod scope;
#[cfg(test)]
mod tests;
mod walk;

use overt_diag::DiagCx;
use overt_index::IndexVec;
use overt_intern::{Interner, Symbol};

use crate::tree::{BinderId, ExprId, Hir};
use scope::Scopes;

/// What a name expression refers to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Res {
    Binder(BinderId),
    /// Not declared in this file: a package, a prelude name, or an error found later.
    Outer(Symbol),
}

/// The resolution of every name expression of a file.
pub struct Resolution {
    pub names: IndexVec<ExprId, Option<Res>>,
}

/// Resolves the names of `hir` and reports duplicates, shadowing and irrefutable tests.
pub fn resolve(hir: &Hir, interner: &Interner, cx: &mut DiagCx<'_>) -> Resolution {
    let mut names = IndexVec::with_capacity(hir.exprs.len());
    for _ in 0..hir.exprs.len() {
        names.push(None);
    }
    let mut resolver = walk::Resolver { scopes: Scopes::default(), names, declaring: false, interner, cx };
    if let Some(root) = hir.root {
        crate::visit::Visitor::namespace(&mut resolver, hir, root);
    }
    Resolution { names: resolver.names }
}
