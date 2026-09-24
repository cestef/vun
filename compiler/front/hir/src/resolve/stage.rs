//! Stages of quoted code (spec 03, section 4): each `$` escapes one level, out of the innermost
//! quote around it or, outside every quote, from run time to compile time. A chain of `$` that
//! escapes past every level is `meta.escape`.

use overt_diag::{ChildKind, Diagnostic, Msg, errors};
use overt_grammar::TokenKind;

use super::walk::Resolver;
use crate::tree::{BlockId, ExprId, ExprKind, Hir};
use crate::visit::Visitor;

impl Resolver<'_, '_> {
    /// `#{ block }`: the block is one quote deeper.
    pub(super) fn quoted(&mut self, hir: &Hir, block: BlockId) {
        self.quotes += 1;
        self.block(hir, block);
        self.quotes -= 1;
    }

    /// A chain of `$` starting at `expr`: its operand is as many levels out.
    pub(super) fn escape(&mut self, hir: &Hir, expr: ExprId) {
        let mut operand = expr;
        let mut levels = 0;
        while let ExprKind::Prefix { op: TokenKind::Dollar, operand: inner, .. } = &hir.exprs[operand].kind {
            levels += 1;
            operand = *inner;
        }
        if levels > self.quotes + 1 {
            let msg = Msg::new().text("this ").code("$").text(" has no stage to escape to");
            let help = Msg::new()
                .text("each ")
                .code("$")
                .text(" escapes one quote, and the last one runs at compile time");
            let d = Diagnostic::error(errors::META_ESCAPE, msg).at(hir.exprs[expr].origin, None);
            self.cx.diag.error(d.child(ChildKind::Help, help));
        }
        let quotes = self.quotes;
        self.quotes = quotes.saturating_sub(levels);
        self.expr(hir, operand);
        self.quotes = quotes;
    }
}
