//! Name resolution within a file, in one walk that also runs the checks needing no types
//! (spec 01, sections 3, 4 and 6).
//!
//! Namespaces (the file, `struct` bodies) declare all their members before anything in them is
//! resolved, so their order does not matter. Blocks declare in order. A function literal sees the
//! namespaces around it and the compile-time (`$`) binders of enclosing functions, but not their
//! run-time locals: functions do not capture, closures list what they capture. A name found
//! nowhere in the file is [`Res::Outer`], looked up among the package's names by
//! [`Resolution::check_outer`].

mod declare;
mod outer;
mod scope;
mod stage;
#[cfg(test)]
mod tests;
mod walk;

use vun_diag::DiagCx;
use vun_index::IndexVec;
use vun_intern::Symbol;

use crate::tree::{BinderId, ExprId, Hir, NsId};
pub use outer::Outside;
use scope::Scopes;

/// What a name expression refers to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Res {
    Binder(BinderId),
    /// Not declared in this file: spread into namespace `ns` or one enclosing it, a package, a
    /// prelude name, or an error found later.
    Outer {
        name: Symbol,
        ns: NsId,
    },
}

/// The resolution of every name expression of a file.
pub struct Resolution {
    pub names: IndexVec<ExprId, Option<Res>>,
    /// The namespace enclosing each namespace; `None` for the file.
    pub parents: IndexVec<NsId, Option<NsId>>,
}

impl Hir {
    /// Resolves the names of `hir` and reports duplicates, shadowing, irrefutable tests and `$`
    /// with no stage to escape to.
    pub fn resolve(&self, cx: &mut DiagCx<'_>) -> Resolution {
        let mut resolver = walk::Resolver {
            scopes: Scopes::default(),
            names: IndexVec::filled(None, self.exprs.len()),
            parents: IndexVec::filled(None, self.namespaces.len()),
            namespaces: Vec::new(),
            declaring: false,
            members: false,
            quotes: 0,
            cx,
        };
        if let Some(root) = self.root {
            crate::visit::Visitor::namespace(&mut resolver, self, root);
        }
        Resolution { names: resolver.names, parents: resolver.parents }
    }
}
