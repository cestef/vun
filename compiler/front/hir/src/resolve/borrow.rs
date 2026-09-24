//! Borrow modes: the words after `&` must name a row of the table `borrows` (appendix B).

use vun_desugar::Borrow;
use vun_diag::{ChildKind, Diagnostic, Msg, errors};
use vun_grammar::TokenKind;

use super::walk::Resolver;
use crate::tree::{ExprId, Hir};

impl Resolver<'_, '_> {
    /// Reports the borrow `expr`, whose words name no mode.
    pub(super) fn unknown_borrow(&mut self, hir: &Hir, expr: ExprId) {
        let amp = TokenKind::Amp.text().unwrap_or_default();
        let modes =
            Borrow::all().map(|b| if b.is_plain() { amp.to_owned() } else { [amp, &b.to_string()].concat() });
        let msg = Msg::new().text("the words after ").code(amp).text(" name no borrow mode");
        let help = Msg::new().text("the modes are ").code_list(modes);
        let d = Diagnostic::error(errors::SYNTAX_UNKNOWN_BORROW, msg).at(hir.exprs[expr].origin, None);
        self.cx.diag.error(d.child(ChildKind::Help, help));
    }
}
