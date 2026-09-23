//! Parser diagnostics: one report per position, placed where the absent thing belongs.

use overt_diag::{ChildKind, DiagId, Diagnostic, Msg, errors};
use overt_grammar::TokenKind;
use overt_index::Idx;
use overt_span::{ByteRange, Span};

use super::{List, Parser};
use crate::tokens::TokenId;

impl Parser<'_, '_, '_> {
    pub(super) fn report(&mut self, span: Span, d: Diagnostic) {
        // One position reports once, and the lexer already reported its malformed tokens.
        if self.reported == Some(self.pos) || self.tokens.diagnosed(self.current()) {
            return;
        }
        self.reported = Some(self.pos);
        self.cx.error_at(span, d);
    }

    pub(super) fn report_here(&mut self, id: DiagId, msg: Msg) {
        let span = self.tokens.span(self.current());
        self.report(span, Diagnostic::error(id, msg));
    }

    pub(super) fn report_expected(&mut self, expected: &[TokenKind]) {
        let texts = expected.iter().map(|k| k.text().unwrap_or_default());
        let msg = self.found(Msg::new().text("expected ").code_list(texts).text(", found "));
        self.report_missing(errors::SYNTAX_EXPECTED_TOKEN, msg);
    }

    /// Reports something absent right after the previous token, where it belongs.
    pub(super) fn report_missing(&mut self, id: DiagId, msg: Msg) {
        self.report(self.gap(), Diagnostic::error(id, msg));
    }

    /// The current or previous token, if it is a keyword that may have been meant as a name.
    pub(super) fn nearby_keyword(&self) -> Option<TokenId> {
        let previous = self.pos.checked_sub(1).map(TokenId::new);
        [Some(self.current()), previous]
            .into_iter()
            .flatten()
            .find(|&t| TokenKind::KEYWORDS.contains(&self.kind(t)))
    }

    /// The empty span right after the previous token, where something absent belongs.
    pub(super) fn gap(&self) -> Span {
        match self.pos.checked_sub(1) {
            Some(previous) => {
                let end = self.tokens.range(TokenId::new(previous)).hi;
                Span { file: self.tokens.file(), range: ByteRange::new(end, end) }
            }
            None => self.tokens.span(self.current()),
        }
    }

    pub(super) fn report_expected_entry(&mut self, list: List) {
        let (id, what) = match list {
            List::Members => (errors::SYNTAX_EXPECTED_DECLARATION, "expected a declaration, found "),
            List::Statements | List::Items(_) => {
                (errors::SYNTAX_EXPECTED_EXPRESSION, "expected an expression, found ")
            }
        };
        let msg = self.found(Msg::new().text(what));
        self.report_here(id, msg);
    }

    pub(super) fn report_chained(&mut self) {
        let op = self.text(self.current()).to_owned();
        let msg = Msg::new().code(op).text(" cannot follow an operator of the same precedence");
        let help = Msg::new().text("add parentheses, or join comparisons with ").code("and");
        let d = Diagnostic::error(errors::SYNTAX_CHAINED_OPERATOR, msg).child(ChildKind::Help, help);
        let span = self.tokens.span(self.current());
        self.report(span, d);
    }

    /// Appends the current token to `msg`.
    pub(super) fn found(&self, msg: Msg) -> Msg {
        match self.peek() {
            TokenKind::Eof => msg.text("end of file"),
            _ => msg.code(self.text(self.current())),
        }
    }
}
