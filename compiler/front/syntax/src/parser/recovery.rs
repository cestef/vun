//! Skipping tokens after an error, so that the tree keeps its shape.

use vun_grammar::{NodeKind, TokenKind};

use super::Parser;
use crate::tokens::TokenId;

impl Parser<'_, '_, '_> {
    /// Skips tokens as `Error` leaves up to one in `stops`, or a closing delimiter, outside any
    /// group opened while skipping.
    pub(super) fn recover(&mut self, stops: &[TokenKind]) {
        let mut depth = 0usize;
        loop {
            let k = self.peek();
            if k == TokenKind::Eof || (depth == 0 && (stops.contains(&k) || k.closes())) {
                return;
            }
            if k.opens() {
                depth += 1;
            } else if k.closes() {
                depth -= 1;
            }
            self.leaf(NodeKind::Error);
        }
    }

    /// Takes the current token, and the rest of its group, as `Error` leaves, when it opens one.
    pub(super) fn bump_group(&mut self) -> TokenId {
        let opens = self.peek().opens();
        let token = self.bump();
        if opens {
            self.recover(&[]);
            if !self.at(TokenKind::Eof) {
                self.leaf(NodeKind::Punct);
            }
        }
        token
    }
}
