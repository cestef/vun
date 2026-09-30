//! Parser diagnostics: one report per position, placed where the absent thing belongs.

use vun_diag::{Diagnostic, Fix, Msg, Safety, errors};
use vun_grammar::TokenKind;
use vun_index::Idx;
use vun_span::{ByteRange, Span};
use vun_trace::tracing;

use super::Parser;
use crate::lexer::ESCAPE;
use crate::tokens::TokenId;

impl Parser<'_, '_, '_> {
    pub(super) fn report(&mut self, span: Span, d: Diagnostic) {
        // One position reports once, the lexer already reported its malformed tokens, and what
        // goes wrong before the productions read on in place follows from the last error.
        let fallout = self.quiet || self.reported == Some(self.pos) || self.tokens.diagnosed(self.current());
        self.quiet = true;
        tracing::trace!(at = self.pos, id = ?d.id, fallout, "report");
        if fallout {
            return;
        }
        self.reported = Some(self.pos);
        self.cx.error_at(span, d);
    }

    pub(super) fn report_expected(&mut self, expected: &[TokenKind]) {
        let texts = expected.iter().map(|k| k.text().unwrap_or_default());
        let msg = self.found(Msg::new().text("expected ").code_choices(texts).text(", found "));
        let mut d = Diagnostic::error(errors::SYNTAX_EXPECTED_TOKEN, msg);
        if let [only] = expected
            && let Some(text) = only.text()
        {
            d = d.fix(self.insertion(text));
        }
        self.report(self.gap(), d);
    }

    /// A fix inserting `text` where something absent belongs. Unsafe: the parser cannot know that
    /// the insertion is what was meant.
    pub(super) fn insertion(&self, text: &str) -> Fix {
        let msg = Msg::new().text("insert ").code(text);
        Fix { preferred: true, ..Fix::replace(msg, Safety::Unsafe, self.gap(), text) }
    }

    /// A fix escaping the keyword `token` so that it reads as a name.
    pub(super) fn escape(&self, token: TokenId) -> Fix {
        let escaped = format!("{ESCAPE}{}", self.text(token));
        let msg = Msg::new().text("to use a keyword as a name, escape it: ").code(escaped.clone());
        Fix::replace(msg, Safety::Unsafe, self.tokens.span(token), escaped)
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

    /// Reports the group `open` opened, unclosed before the current token.
    pub(super) fn report_unclosed(&mut self, open: TokenId, close: TokenKind) {
        let msg = Msg::new().text("unclosed ").code(self.text(open).to_owned());
        let closer = close.text().unwrap_or_default();
        let help = Msg::new().text("expected ").code(closer).text(", found ");
        let d = Diagnostic::error(errors::SYNTAX_UNCLOSED_DELIMITER, msg)
            .help(self.found(help))
            .fix(self.insertion(closer));
        self.report(self.tokens.span(open), d);
    }

    pub(super) fn report_chained(&mut self) {
        let op = self.text(self.current()).to_owned();
        let msg = Msg::new().code(op).text(" cannot follow an operator of the same precedence");
        let help = Msg::new().text("add parentheses, or join comparisons with ").code("and");
        let d = Diagnostic::error(errors::SYNTAX_CHAINED_OPERATOR, msg).help(help);
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
