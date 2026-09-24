//! The token cursor: looking ahead, consuming tokens and pushing leaves.

use overt_grammar::{Context, NodeKind, OPERANDS, PREFIX, TokenKind};
use overt_index::Idx;

use super::Parser;
use crate::cst::Anchor;
use crate::tokens::TokenId;

impl Parser<'_, '_, '_> {
    pub(super) fn current(&self) -> TokenId {
        TokenId::new(self.pos)
    }

    pub(super) fn nth(&self, ahead: usize) -> TokenKind {
        let last = self.tokens.len() - 1;
        self.tokens.kind(TokenId::new((self.pos + ahead).min(last)))
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

    pub(super) fn at_word(&self, context: Context) -> bool {
        self.at(TokenKind::Ident) && context.keyword(self.text(self.current()))
    }

    pub(super) fn starts_expr(&self) -> bool {
        self.starts_expr_at(0)
    }

    /// Whether the token `ahead` of the current one can start an expression.
    pub(super) fn starts_expr_at(&self, ahead: usize) -> bool {
        let k = self.nth(ahead);
        PREFIX.contains(&k) || OPERANDS.contains(&k)
    }

    /// Consumes the current token, which some node must anchor. After a token the lexer
    /// reported, the next position stays quiet: what follows is usually its fallout.
    pub(super) fn bump(&mut self) -> TokenId {
        debug_assert!(!self.at(TokenKind::Eof), "Eof anchors the file node only");
        let t = self.current();
        self.pos += 1;
        if self.tokens.diagnosed(t) {
            self.reported = Some(self.pos);
        }
        t
    }

    pub(super) fn leaf(&mut self, kind: NodeKind) {
        let mark = self.out.mark();
        let token = self.bump();
        self.out.node(kind, Anchor::Token(token), mark);
    }

    pub(super) fn missing(&mut self, kind: NodeKind) {
        let mark = self.out.mark();
        self.out.node(kind, Anchor::Missing(self.current()), mark);
    }

    pub(super) fn eat(&mut self, kind: TokenKind) -> bool {
        let found = self.at(kind);
        if found {
            self.leaf(NodeKind::Punct);
        }
        found
    }

    pub(super) fn expect(&mut self, kind: TokenKind) {
        if !self.eat(kind) {
            self.report_expected(&[kind]);
            self.missing(NodeKind::Punct);
        }
    }
}
