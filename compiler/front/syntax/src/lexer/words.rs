//! Comments, words and punctuation.

use vun_diag::{Msg, errors};
use vun_grammar::TokenKind;

use super::{DOC_COMMENT, ESCAPE, Lexer, MODULE_DOC_COMMENT, is_ident_continue, is_ident_start};

impl Lexer<'_, '_, '_> {
    pub(super) fn comment(&mut self) -> TokenKind {
        let kind = if self.at(MODULE_DOC_COMMENT) {
            TokenKind::ModuleDocComment
        } else if self.at(DOC_COMMENT) && !self.rest()[DOC_COMMENT.len()..].starts_with('/') {
            TokenKind::DocComment
        } else {
            TokenKind::Comment
        };
        self.eat_line();
        kind
    }

    pub(super) fn word(&mut self) -> TokenKind {
        let lo = self.pos;
        self.eat_while(is_ident_continue);
        let word = &self.text[lo..self.pos];
        TokenKind::keyword(word).unwrap_or(if TokenKind::Underscore.text() == Some(word) {
            TokenKind::Underscore
        } else {
            TokenKind::Ident
        })
    }

    pub(super) fn punctuation(&mut self) -> TokenKind {
        if let Some(kind) = TokenKind::punctuation(self.rest()) {
            self.pos += kind.text().map_or(0, str::len);
            return kind;
        }
        let lo = self.pos;
        self.bump();
        while self.pos < self.text.len() && !self.starts_token() {
            self.bump();
        }
        let msg = Msg::new().text("not a valid token: ").code(&self.text[lo..self.pos]);
        self.error(lo, errors::SYNTAX_UNKNOWN_CHARACTER, msg);
        TokenKind::Error
    }

    pub(super) fn starts_token(&self) -> bool {
        self.peek().is_some_and(|c| {
            c.is_ascii_whitespace()
                || is_ident_start(c)
                || c.is_ascii_digit()
                || matches!(c, '"' | '\'' | ESCAPE)
        }) || TokenKind::punctuation(self.rest()).is_some()
    }
}
