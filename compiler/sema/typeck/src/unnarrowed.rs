//! Why a comparison did not narrow what it compared (spec 04, rule 4): a mutable place keeps its
//! range, since any write would undo what the test learned. A value that does not fit because of
//! it is told to bind the place first, which does narrow.

use vun_desugar::COMPARISONS;
use vun_hir::tree::{ExprId, ExprKind, Hir};
use vun_hir::visit::{Visitor, walk_expr};
use vun_origin::OriginId;

use vun_diag::Msg;

use super::Typer;

/// The members read in an expression.
struct Places(Vec<ExprId>);

impl Visitor for Places {
    fn expr(&mut self, hir: &Hir, expr: ExprId) {
        if matches!(hir.exprs[expr].kind, ExprKind::Member { .. }) {
            self.0.push(expr);
        }
        walk_expr(self, hir, expr);
    }
}

impl Typer<'_, '_, '_> {
    /// What to do about the mutable `place`, compared above without being narrowed (spec 04,
    /// rule 4): a binding of it is narrowed.
    pub(super) fn unnarrowed(place: &str) -> Msg {
        let name = place.rsplit('.').next().unwrap_or(place).trim_start_matches('_');
        Msg::new()
            .code(place)
            .text(" is a mutable place, which a comparison does not narrow; bind it first, ")
            .code(format!("{name} := {place};"))
            .text(", and use the binding")
    }

    /// The expression written at `at`.
    pub(super) fn written_at(&self, at: OriginId) -> Option<ExprId> {
        self.hir().exprs.iter().find(|(_, e)| e.origin == at).map(|(e, _)| e)
    }

    /// The place, as written, that the value written at `at` reads and that a comparison in the
    /// same function compares without narrowing it: a field, which is mutable.
    pub(super) fn compared_place(&self, at: OriginId) -> Option<String> {
        let hir = self.hir();
        let value = self.written_at(at)?;
        let mut reads = Places(Vec::new());
        walk_expr(&mut reads, hir, value);
        let read: Vec<String> = reads.0.iter().map(|&p| self.env().source(p)).collect();
        let function = hir.enclosing_function(value);
        let mut comparisons = hir.exprs.iter().filter(|(e, _)| hir.enclosing_function(*e) == function);
        comparisons.find_map(|(_, node)| {
            let ExprKind::Infix { op, left, right: Some(right) } = node.kind else { return None };
            COMPARISONS.iter().find(|row| row.token == op)?;
            [left, right]
                .into_iter()
                .filter(|&operand| matches!(hir.exprs[operand].kind, ExprKind::Member { .. }))
                .map(|operand| self.env().source(operand))
                .find(|place| read.contains(place))
        })
    }
}
