//! Declaring binders: duplicates and shadowing are reported as each name is declared.

use overt_diag::{ChildKind, Diagnostic, Label, Msg, errors};

use super::scope::Kind;
use super::walk::Resolver;
use crate::tree::{BinderId, Entry, Hir, List, PatId, PatKind, Staged};
use crate::visit::Visitor;

impl Resolver<'_, '_, '_> {
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
        let msg = Msg::new().code(self.interner.resolve(name)).text(what);
        let mut d = Diagnostic::error(id, msg).at(hir.binders[binder].origin, None);
        d.labels.push(Label { origin: hir.binders[earlier].origin, msg: Some("first bound here".into()) });
        let help = Msg::new().text("choose another name");
        self.cx.diag.error(d.child(ChildKind::Help, help));
    }

    /// Binders introduced by a pattern, without visiting the expressions inside it.
    pub(super) fn binders(hir: &Hir, pat: PatId, out: &mut Vec<BinderId>) {
        match &hir.pats[pat].kind {
            PatKind::Bind(b) | PatKind::Rest(Some(b)) => out.push(*b),
            PatKind::At { binder, pat } => {
                out.push(*binder);
                Self::binders(hir, *pat, out);
            }
            PatKind::Tuple(items) => each_entry(items, &mut |p| Self::binders(hir, *p, out)),
            PatKind::Variant { args: Some(args), .. } => {
                each_entry(args, &mut |p| Self::binders(hir, *p, out));
            }
            PatKind::Struct { fields, .. } => each_entry(fields, &mut |(_, p)| Self::binders(hir, *p, out)),
            // Alternatives bind the same names; the first stands for all.
            PatKind::Or(alternatives) => {
                if let Some(&first) = alternatives.first() {
                    Self::binders(hir, first, out);
                }
            }
            PatKind::Variant { args: None, .. }
            | PatKind::Rest(None)
            | PatKind::Hole
            | PatKind::Range { .. }
            | PatKind::Value(_)
            | PatKind::Error => {}
        }
    }

    /// Visits a pattern that declares its binders here.
    pub(super) fn bind_pattern(&mut self, hir: &Hir, pat: PatId) {
        let declaring = std::mem::replace(&mut self.declaring, true);
        self.pat(hir, pat);
        self.declaring = declaring;
    }
}

pub(super) fn each_entry<T>(list: &List<T>, f: &mut dyn FnMut(&T)) {
    for e in list {
        match e {
            Entry::One(item) => f(item),
            Entry::Staged(Staged::For { body, .. }) => each_entry(body, f),
            Entry::Staged(Staged::If { then, otherwise, .. }) => {
                each_entry(then, f);
                each_entry(otherwise, f);
            }
        }
    }
}
