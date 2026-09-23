//! The names a pattern or a namespace introduces.

use super::{BinderId, Entry, Hir, List, Member, NsId, PatId, PatKind, Staged};

impl Hir {
    /// Binders introduced by a pattern, without visiting the expressions inside it.
    pub fn binders(&self, pat: PatId, out: &mut Vec<BinderId>) {
        match &self.pats[pat].kind {
            PatKind::Bind(b) | PatKind::Rest(Some(b)) => out.push(*b),
            PatKind::At { binder, pat } => {
                out.push(*binder);
                self.binders(*pat, out);
            }
            PatKind::Tuple(items) => each_entry(items, &mut |p| self.binders(*p, out)),
            PatKind::Variant { args: Some(args), .. } => each_entry(args, &mut |p| self.binders(*p, out)),
            PatKind::Struct { fields, .. } => each_entry(fields, &mut |(_, p)| self.binders(*p, out)),
            // Alternatives bind the same names; the first stands for all.
            PatKind::Or(alternatives) => {
                if let Some(&first) = alternatives.first() {
                    self.binders(first, out);
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

    /// The binders declared by the members of a namespace, including staged ones.
    #[must_use]
    pub fn members(&self, ns: NsId) -> Vec<BinderId> {
        let mut out = Vec::new();
        each_entry(&self.namespaces[ns].members, &mut |m| match m {
            Member::Decl(d) => self.binders(self.decls[*d].pat, &mut out),
            Member::Spread(s) => out.extend(s.select.iter().flatten()),
            Member::Field(_) => {}
        });
        out
    }
}

/// Calls `f` on every entry of `list`, entering both branches of `$if` and the body of `$for`.
pub fn each_entry<'a, T>(list: &'a List<T>, f: &mut dyn FnMut(&'a T)) {
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
