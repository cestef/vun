//! An integer that does not fit the type expected (spec 04, rule 2).

use vun_desugar::{COMPOUND, Narrowing};
use vun_diag::{ChildKind, Diagnostic, Msg, errors};
use vun_origin::OriginId;
use vun_sema_ty::Ty;

use super::{Known, Stage, Typer};

impl Typer<'_, '_, '_> {
    /// The report of `found`, written at `at`, not within `expected`, which is written as `written`.
    pub(super) fn does_not_fit(
        &self,
        at: OriginId,
        (found, expected): (Known, Ty),
        written: &str,
    ) -> Diagnostic {
        let msg = Msg::new().text("value does not fit the type ").code(written);
        let note = Msg::new().text("the value is ").ty(found.ty.to_string());
        // A type written as an alias is shown with what it stands for; otherwise it is said.
        let note = if written == expected.to_string() {
            note.text(", which is not within ").code(written)
        } else {
            note.text(", and ").code(written).text(" is ").ty(expected.to_string())
        };
        // A run-time value can be narrowed; a constant is changed, or the type widened.
        let help = if found.stage == Stage::Run {
            // The compound assignments that narrow without trapping, from the table `compound`.
            let operators = [Narrowing::Wrap, Narrowing::Sat].map(|n| {
                COMPOUND
                    .iter()
                    .find(|row| row.narrowing == n)
                    .and_then(|row| row.token.text())
                    .unwrap_or_default()
            });
            Msg::new()
                .text("widen ")
                .code(written)
                .text(" to hold the whole range, or narrow the value with ")
                .code_list([".to", ".wrap", ".sat"])
                .text("; counters can narrow as they update, with ")
                .code_list(operators)
        } else {
            Msg::new()
                .text("use a value within ")
                .code(written)
                .text(", or widen it if it is meant to hold this one")
        };
        let d = Diagnostic::error(errors::INT_DOES_NOT_FIT, msg)
            .child(ChildKind::Note, note)
            .child(ChildKind::Help, help);
        match self.compared_place(at) {
            Some(place) => d.child(ChildKind::Help, Self::unnarrowed(&place)),
            None => d,
        }
    }

    /// What the label of a value says of its type: the bits an integer range needs against the
    /// bits the type expected holds, which a range such as `0..=12884901885` does not show; the
    /// type otherwise.
    pub(super) fn shown(found: Ty, expected: Ty, written: &str) -> Msg {
        let bits =
            |ty: Ty| ty.interval().filter(|range| !range.is_singleton()).and_then(|range| range.bits());
        match (bits(found), expected.interval().and_then(|range| range.bits())) {
            (Some(needs), Some(holds)) => Msg::new()
                .text("this needs ")
                .code(format!("{needs} bits"))
                .text(", ")
                .code(written)
                .text(" holds ")
                .code(format!("{holds} bits")),
            _ => Msg::new().text("this is ").ty(found.to_string()),
        }
    }
}
