//! `..` as a whole value (spec 01, section 6): it skips the rest of a pattern, so a value
//! position holding it is `binding.rest_value`.

use vun_diag::{Diagnostic, Msg, errors};
use vun_grammar::TokenKind;
use vun_hir::tree::{ExprId, ExprKind, Hir};

use super::Checker;

impl Checker<'_, '_, '_> {
    /// Reports `expr` when it is `..` where a value is written.
    pub(super) fn rest(&mut self, hir: &Hir, expr: ExprId) {
        if !matches!(hir.exprs[expr].kind, ExprKind::Rest) {
            return;
        }
        let rest = TokenKind::DotDot.text().unwrap_or_default();
        let msg = Msg::new().code(rest).text(" skips the rest of a pattern and is no value");
        let help = Msg::new()
            .text("write the bounds of a range, such as ")
            .code("a..b")
            .text(" or ")
            .code("a..")
            .text(", where a value is meant");
        self.diag.error(
            Diagnostic::error(errors::BINDING_REST_VALUE, msg).at(hir.exprs[expr].origin, None).help(help),
        );
    }
}
