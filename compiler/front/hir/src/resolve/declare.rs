//! Declaring binders: duplicates and shadowing are reported as each name is declared.

use overt_diag::{ChildKind, Diagnostic, Label, Msg, errors};

use super::scope::Kind;
use super::walk::Resolver;
use crate::tree::{BinderId, Hir, PatId};
use crate::visit::Visitor;

impl Resolver<'_, '_> {
    /// Declares a binder in the innermost scope. A second member of the same name in one namespace
    /// is a duplicate; a local reusing any name visible in the file shadows it. Members of nested
    /// namespaces may reuse outer names: they are reached through their namespace.
    pub(super) fn declare(&mut self, hir: &Hir, binder: BinderId) {
        let name = hir.binders[binder].name;
        let (id, earlier, what) = match self.scopes.bind(name, binder) {
            Some(previous) if self.scopes.innermost() == Some(Kind::Namespace) => {
                (errors::NAMES_DUPLICATE, previous, " is declared twice in this namespace")
            }
            Some(previous) => {
                (errors::NAMES_SHADOWED, previous, " is already bound; shadowing is not allowed")
            }
            None if !self.scopes.innermost().is_some_and(Kind::checks_shadowing) => return,
            None => match self.scopes.lookup_outer(hir, name) {
                Some(previous) => {
                    (errors::NAMES_SHADOWED, previous, " is already bound; shadowing is not allowed")
                }
                None => return,
            },
        };
        let msg = Msg::new().code(name.as_str()).text(what);
        let mut d = Diagnostic::error(id, msg).at(hir.binders[binder].origin, None);
        d.labels.push(Label { origin: hir.binders[earlier].origin, msg: Some("first bound here".into()) });
        let help = Msg::new().text("choose another name");
        self.cx.diag.error(d.child(ChildKind::Help, help));
    }

    /// Visits a pattern that declares its binders here.
    pub(super) fn bind_pattern(&mut self, hir: &Hir, pat: PatId) {
        let declaring = std::mem::replace(&mut self.declaring, true);
        self.pat(hir, pat);
        self.declaring = declaring;
    }
}
