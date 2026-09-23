//! Skipping tokens after an error, so that the tree keeps its shape.

use overt_grammar::{NodeKind, TokenKind};

use super::Parser;
use crate::cst::Anchor;

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

    /// Skips the current token as an `Error` node, holding the rest of its group if it opens one.
    pub(super) fn skip_group(&mut self) {
        if self.at(TokenKind::Eof) {
            self.missing(NodeKind::Error);
            return;
        }
        let mark = self.out.mark();
        let opens = self.peek().opens();
        let token = self.bump();
        if opens {
            self.recover(&[]);
            if !self.at(TokenKind::Eof) {
                self.leaf(NodeKind::Punct);
            }
        }
        self.out.node(NodeKind::Error, Anchor::Token(token), mark);
    }
}
