//! Names not declared in the file: spread into one of its namespaces, the package itself, its
//! dependencies and its prelude.

use vun_diag::{ChildKind, DiagCx, Diagnostic, ErrorGuaranteed, Msg, errors};
use vun_intern::Symbol;
use vun_suggest::closest;

use super::{Res, Resolution};
use crate::tree::{Hir, NsId};

/// Where names from outside a file are found; the driver knows packages and their namespaces.
pub trait Outside {
    /// Whether `name` is spread into `ns`, or, for `None`, whether the package, a dependency or
    /// the prelude provides it.
    fn contains(&self, name: Symbol, ns: Option<NsId>) -> bool;
    /// The names [`Self::contains`] accepts, for suggestions.
    fn names(&self, ns: Option<NsId>) -> Vec<Symbol>;
    /// Whether some names meant for `ns` (or, for `None`, for the package) could not be read,
    /// which is reported; a name found nowhere may be one of them.
    fn poisoned(&self, ns: Option<NsId>) -> Option<ErrorGuaranteed>;
}

impl Resolution {
    /// The namespaces from `ns` outwards, then `None` for the package.
    pub fn enclosing(&self, ns: NsId) -> impl Iterator<Item = Option<NsId>> + '_ {
        std::iter::successors(Some(Some(ns)), |&n| n.map(|n| self.parents[n])).chain([None])
    }

    /// Reports every [`Res::Outer`] name that `outside` does not provide, suggesting the closest
    /// name visible there or declared at the file's top level. Where `outside` is poisoned,
    /// nothing is reported.
    pub fn check_outer(&self, hir: &Hir, outside: &dyn Outside, cx: &mut DiagCx<'_>) {
        for (expr, res) in self.names.iter() {
            let Some(Res::Outer { name, ns }) = *res else { continue };
            if self.enclosing(ns).any(|n| outside.contains(name, n) || outside.poisoned(n).is_some()) {
                continue;
            }
            let top = hir.root.map(|root| hir.members(root)).unwrap_or_default();
            let visible = self.enclosing(ns).flat_map(|n| outside.names(n));
            let candidates: Vec<&str> =
                visible.chain(top.iter().map(|&b| hir.binders[b].name)).map(Symbol::as_str).collect();
            let text = name.as_str();
            let msg = Msg::new().text("cannot find ").code(text).text(" in scope");
            let mut d = Diagnostic::error(errors::NAMES_UNRESOLVED, msg).at(hir.exprs[expr].origin, None);
            if let Some(near) = closest(text, candidates.iter().copied()) {
                d = d.child(ChildKind::Help, Msg::did_you_mean(near));
            }
            cx.diag.error(d);
        }
    }
}
