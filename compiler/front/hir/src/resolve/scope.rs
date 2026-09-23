//! The stack of scopes during resolution.

use std::collections::HashMap;

use overt_intern::Symbol;

use crate::tree::{BinderId, Hir};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    /// A file or a `struct` body: order-independent.
    Namespace,
    /// A block, an arm, a loop or a test: declarations in order.
    Local,
    /// The parameters of a function literal; locals below it are hidden, except compile-time ones.
    Function,
    /// The parameters of a function type: names for later parameter types, which shadow nothing.
    Signature,
}

impl Kind {
    /// Whether scopes below this one lose sight of their run-time locals.
    fn hides_locals(self) -> bool {
        matches!(self, Self::Function | Self::Signature)
    }

    /// Whether a binder declared here may not reuse a name visible from enclosing scopes.
    pub(super) fn checks_shadowing(self) -> bool {
        matches!(self, Self::Local | Self::Function)
    }
}

struct Scope {
    kind: Kind,
    names: HashMap<Symbol, BinderId>,
}

#[derive(Default)]
pub(super) struct Scopes {
    stack: Vec<Scope>,
}

impl Scopes {
    pub(super) fn push(&mut self, kind: Kind) {
        self.stack.push(Scope { kind, names: HashMap::new() });
    }

    pub(super) fn pop(&mut self) {
        self.stack.pop();
    }

    /// Binds `name` in the innermost scope, returning the binder it replaces in that scope.
    pub(super) fn bind(&mut self, name: Symbol, binder: BinderId) -> Option<BinderId> {
        self.stack.last_mut().and_then(|s| s.names.insert(name, binder))
    }

    /// The binder `name` refers to here. Crossing a function boundary hides run-time locals.
    pub(super) fn lookup(&self, hir: &Hir, name: Symbol) -> Option<BinderId> {
        self.lookup_from(hir, name, 0)
    }

    /// The binder `name` refers to from the scope enclosing the innermost one.
    pub(super) fn lookup_outer(&self, hir: &Hir, name: Symbol) -> Option<BinderId> {
        self.lookup_from(hir, name, 1)
    }

    fn lookup_from(&self, hir: &Hir, name: Symbol, skip: usize) -> Option<BinderId> {
        let mut crossed = self.stack.iter().rev().take(skip).any(|s| s.kind.hides_locals());
        for scope in self.stack.iter().rev().skip(skip) {
            if let Some(&b) = scope.names.get(&name) {
                let visible = !crossed || scope.kind != Kind::Local || hir.binders[b].comptime;
                if visible {
                    return Some(b);
                }
            }
            crossed |= scope.kind.hides_locals();
        }
        None
    }

    pub(super) fn innermost(&self) -> Option<Kind> {
        self.stack.last().map(|s| s.kind)
    }
}
