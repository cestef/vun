//! How deep a tree may grow (`MAX_NESTING`): every operand and block is one level, and so is each
//! link of a chain wrapping the node before it (`a + b + c`, `x.a.b`, `else if`). Past the limit
//! the rest of the expression is one `Error` node, reported once, so no later stage walks a tree
//! deeper than the limit.

use vun_diag::{ChildKind, Diagnostic, Msg, errors};
use vun_grammar::{NodeKind, TokenKind};

use super::{MAX_NESTING, Parser};
use crate::cst::{Anchor, Mark};

/// Where skipping the rest of an expression stops, besides a closing delimiter.
const EXPRESSION_ENDS: &[TokenKind] = &[TokenKind::Semi, TokenKind::Comma];

/// A chain of links, each wrapping the node since `mark`.
pub(super) struct Chain {
    mark: Mark,
    links: u32,
}

impl Chain {
    pub(super) fn new(mark: Mark) -> Self {
        Self { mark, links: 0 }
    }
}

impl Parser<'_, '_, '_> {
    /// Runs `body` one level deeper, or, when nesting is too deep, skips the rest of the
    /// expression.
    pub(super) fn nested(&mut self, body: impl FnOnce(&mut Self)) {
        if self.depth >= MAX_NESTING {
            let mark = self.out.mark();
            self.too_deep(mark);
            return;
        }
        self.depth += 1;
        body(self);
        self.depth -= 1;
    }

    /// Whether `chain` may take one more link; when it is too deep, the rest of the expression
    /// is skipped, and the chain ends.
    pub(super) fn link(&mut self, chain: &mut Chain) -> bool {
        chain.links += 1;
        if self.depth.saturating_add(chain.links) < MAX_NESTING {
            return true;
        }
        self.too_deep(chain.mark);
        false
    }

    /// Reports nesting past the limit at the current token, and makes the tree since `mark` and
    /// the rest of the expression one `Error` node.
    fn too_deep(&mut self, mark: Mark) {
        let msg = Msg::new().text("nesting is deeper than ").num(MAX_NESTING).text(" levels");
        let note = Msg::new().text("each operand, each block and each link of a chain is one level");
        let span = self.tokens.span(self.current());
        let d = Diagnostic::error(errors::SYNTAX_NESTING_TOO_DEEP, msg).child(ChildKind::Note, note);
        self.report(span, d);
        // At the end of the expression, the tree since `mark` stays as it is, within the limit.
        if self.at(TokenKind::Eof) || EXPRESSION_ENDS.contains(&self.peek()) || self.peek().closes() {
            if mark == self.out.mark() {
                self.missing(NodeKind::Error);
            }
            return;
        }
        // The current token anchors the node, with its group when it opens one.
        let token = self.bump_group();
        self.recover(EXPRESSION_ENDS);
        self.out.node(NodeKind::Error, Anchor::Token(token), mark);
        // What the constructs around it miss where the skipping ends is part of this report.
        self.reported = Some(self.pos);
    }
}
