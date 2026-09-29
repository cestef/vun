//! The token cursor: looking ahead and consuming tokens.

use vun_grammar::TokenKind;
use vun_index::Idx;

use super::Parser;
use crate::tokens::TokenId;

impl Parser<'_, '_, '_> {
    pub(super) fn current(&self) -> TokenId {
        TokenId::new(self.pos)
    }

    /// The token `ahead` of the current one, the last when past the end.
    pub(super) fn current_plus(&self, ahead: usize) -> TokenId {
        TokenId::new((self.pos + ahead).min(self.tokens.len() - 1))
    }

    pub(super) fn nth(&self, ahead: usize) -> TokenKind {
        self.tokens.kind(self.current_plus(ahead))
    }

    pub(super) fn peek(&self) -> TokenKind {
        self.nth(0)
    }

    pub(super) fn kind(&self, t: TokenId) -> TokenKind {
        self.tokens.kind(t)
    }

    pub(super) fn text(&self, t: TokenId) -> &str {
        self.tokens.range(t).of(self.source)
    }

    pub(super) fn at(&self, kind: TokenKind) -> bool {
        self.peek() == kind
    }

    /// Consumes the current token, which some node must anchor or hold. After a token the lexer
    /// reported, the parser stays quiet: what follows is usually its fallout.
    pub(super) fn bump(&mut self) -> TokenId {
        debug_assert!(!self.at(TokenKind::Eof), "Eof anchors the file node only");
        let t = self.current();
        self.pos += 1;
        if self.tokens.diagnosed(t) {
            self.reported = Some(self.pos);
            self.quiet = true;
        }
        t
    }
}
