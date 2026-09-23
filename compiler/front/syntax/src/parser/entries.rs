//! Entries of member and statement lists: declarations, fields, assignments, values.

use overt_grammar::{ASSIGN, NodeKind, TokenKind};
use overt_index::Idx;

use super::{Brace, End, List, Parser};
use crate::cst::{Anchor, Mark};
use crate::tokens::TokenId;

impl Parser<'_, '_, '_> {
    pub(super) fn file(&mut self) {
        let mark = self.out.mark();
        self.list(List::Members, TokenKind::Eof);
        self.out.node(NodeKind::File, Anchor::Token(self.current()), mark);
    }

    /// One entry of a member or statement list, with its terminator.
    pub(super) fn entry(&mut self, list: List, close: TokenKind) {
        if self.at(TokenKind::ModuleDocComment) && list == List::Members {
            self.leaf(NodeKind::ModuleDoc);
            return;
        }
        if self.eat(TokenKind::Semi) {
            return;
        }
        match self.annotated(|p| p.entry_body(list, close)) {
            End::Semi => self.terminator(),
            End::Comma => {
                if !self.eat(TokenKind::Comma) && !self.at(close) {
                    self.report_expected(&[TokenKind::Comma, close]);
                }
            }
            End::Value => {
                if !self.at(close) {
                    self.terminator();
                }
            }
            End::Block => {
                self.eat(TokenKind::Semi);
            }
            End::Recovered => {}
        }
    }

    pub(super) fn entry_body(&mut self, list: List, close: TokenKind) -> End {
        let statements = list == List::Statements;
        if self.block_like_at() && (statements || self.at(TokenKind::Dollar)) {
            self.block_like(list);
            return End::Block;
        }
        let mark = self.out.mark();
        let mutable = self.at(TokenKind::Mut);
        if mutable {
            self.leaf(NodeKind::Mut);
        } else if !self.starts_expr() {
            self.report_expected_entry(list);
            return self.recover_entry(mark);
        }
        self.expr(Brace::Continues);
        match self.peek() {
            TokenKind::ColonEq => {
                let op = self.bump();
                self.expr(Brace::Continues);
                self.out.node(NodeKind::Binding, Anchor::Token(op), mark);
                End::Semi
            }
            TokenKind::Colon => {
                let op = self.bump();
                self.expr(Brace::Continues);
                if self.eat(TokenKind::Eq) {
                    self.expr(Brace::Continues);
                }
                let field = !statements && (self.at(TokenKind::Comma) || self.at(close));
                let kind = if field { NodeKind::Field } else { NodeKind::TypedBinding };
                self.out.node(kind, Anchor::Token(op), mark);
                if field { End::Comma } else { End::Semi }
            }
            k if statements && !mutable && ASSIGN.contains(&k) => {
                let op = self.bump();
                self.expr(Brace::Continues);
                self.out.node(NodeKind::Assign, Anchor::Token(op), mark);
                End::Semi
            }
            _ if statements && !mutable => End::Value,
            _ => {
                self.report_expected(&[TokenKind::ColonEq, TokenKind::Colon]);
                self.recover_entry(mark)
            }
        }
    }

    /// The `;` ending an entry. Tokens left on the same line are junk and skipped up to it; on a
    /// new line the `;` is missing and the next entry starts.
    pub(super) fn terminator(&mut self) {
        if self.eat(TokenKind::Semi) {
            return;
        }
        self.report_expected(&[TokenKind::Semi]);
        if self.on_new_line() || self.at(TokenKind::Eof) {
            self.missing(NodeKind::Punct);
        } else {
            self.recover(&[TokenKind::Semi]);
            if !self.eat(TokenKind::Semi) {
                self.missing(NodeKind::Punct);
            }
        }
    }

    /// Whether a line break separates the current token from the previous one.
    pub(super) fn on_new_line(&self) -> bool {
        let Some(previous) = self.pos.checked_sub(1) else { return true };
        let between = self.tokens.range(TokenId::new(previous)).hi..self.tokens.range(self.current()).lo;
        self.source[between.start as usize..between.end as usize].contains('\n')
    }

    /// Skips the rest of a malformed entry; everything it pushed since `mark` becomes one `Error`,
    /// so that an annotation wraps a single node.
    pub(super) fn recover_entry(&mut self, mark: Mark) -> End {
        self.recover(&[TokenKind::Semi, TokenKind::Comma]);
        if !self.eat(TokenKind::Semi) {
            self.eat(TokenKind::Comma);
        }
        self.out.node(NodeKind::Error, Anchor::Missing(self.current()), mark);
        End::Recovered
    }
}
