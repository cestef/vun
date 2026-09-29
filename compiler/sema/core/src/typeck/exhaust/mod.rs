//! Exhaustive `match` (spec 01, section 6; decision 29): the arms cover every value of the
//! scrutinee's type, enums by variant and integers by range, or the match is an error naming a
//! value it misses. Arms with a guard cover nothing, since the guard may fail. The check is the
//! usefulness of a wildcard against the arms' patterns (Maranget, "Warnings for pattern
//! matching", 2007), over patterns lowered to the shapes of [`Pat`]; what is not known (a type not
//! evaluated, an enum whose variants are generated) counts as covered, so the check reports only
//! what it knows.

mod lower;
mod missing;
mod witness;

use vun_diag::{Diagnostic, Msg, errors};
use vun_hir::tree::{Arm, Entry};
use vun_intern::Symbol;
use vun_origin::OriginId;
use vun_sema_int::Interval;
use vun_sema_ty::Ty;

use crate::typeck::Typer;

/// A pattern as coverage sees it.
#[derive(Clone, Debug)]
pub(super) enum Pat {
    /// A binder, `_`, or what matches every value.
    Any,
    Variant(Symbol, Vec<Pat>),
    /// A tuple or a struct, its parts in the type's order.
    Product(Vec<Pat>),
    Range(Interval),
    Or(Vec<Pat>),
    /// A value coverage cannot split on, such as a string or a float: it covers nothing.
    Opaque,
}

impl Typer<'_, '_, '_> {
    /// Reports a value of `matched` the arms of the `match` at `origin` do not cover.
    pub(in crate::typeck) fn exhaustive(
        &mut self,
        origin: OriginId,
        matched: Option<Ty>,
        arms: &[Entry<Arm>],
    ) {
        let Some(matched) = matched.map(|t| t.borrow().map_or(t, |(_, inner)| inner)) else { return };
        let mut rows = Vec::new();
        for arm in arms {
            let Entry::One(arm) = arm else { return };
            if arm.guard.is_none() {
                rows.push(vec![self.lower(arm.pat, Some(matched))]);
            }
        }
        if let Some(witness) = self.missing(rows, &[Some(matched)]) {
            let missed = witness.into_iter().next().map(|w| w.to_string()).unwrap_or_default();
            let msg = Msg::new().code("match").text(" does not cover ").code(missed);
            let help = Msg::new().text("add an arm for it, or ").code("_ =>").text(" for every value left");
            let d = Diagnostic::error(errors::BINDING_MISSING_CASE, msg).at(origin, None);
            self.diag.error(d.child(vun_diag::ChildKind::Help, help));
        }
    }
}
