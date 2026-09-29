//! The engine reading the productions of `nodes.vun`, each an automaton over its tokens and
//! children (`vun_grammar::Automaton`).
//!
//! A stack of frames reads the tokens: readings of nodes, alternating with the operands they
//! read. A reading keeps every kind the node may still be, each at the positions of its
//! production it stands at; a child is read once for all the positions reading it, and those that
//! may not hold what was read drop. One rule says who reads a token (`reader`): the outermost
//! reading that reads it where it stands, every frame above it able to end; else the innermost
//! operand a form may continue, by precedence and the longest run of tokens; else the top frame
//! ends, or is mended (`mend`). A node's kind is chosen when the tree is written: the first it
//! spells that the position holding it takes (`tree`).

use vun_diag::DiagCx;
use vun_grammar::{KindSet, NodeKind};
use vun_trace::tracing;

use super::Parser;
use crate::cst::Cst;
use crate::tokens::Tokens;

mod expected;
mod finish;
mod frame;
mod lookahead;
mod mend;
mod moves;
mod reader;
mod spec;
mod start;
mod tree;

pub(super) use frame::Frame;
use frame::Reading;
use reader::Decision;
pub(super) use tree::Tree;

impl Cst {
    /// Parses one file by the productions of `nodes.vun`. Diagnostics go to `cx`; the result is
    /// always a complete tree.
    pub fn parse(tokens: &Tokens, source: &str, cx: &mut DiagCx<'_>) -> Self {
        let _span = tracing::debug_span!("parse", file = ?tokens.file(), tokens = tokens.len()).entered();
        let mut parser = Parser::new(tokens, source, cx);
        let root = parser.read_root();
        parser.tree.write(root, None, &mut parser.out, tokens);
        let cst = parser.out.finish();
        tracing::debug!(nodes = cst.len(), "parsed");
        cst
    }
}

impl Parser<'_, '_, '_> {
    /// Reads the tokens as a node of a root kind, whose production reads the end of the file.
    fn read_root(&mut self) -> u32 {
        let roots = NodeKind::ALL.iter().copied().filter(|k| k.is_root()).fold(KindSet::EMPTY, KindSet::with);
        self.stack.push(Frame::Reading(Reading::new(roots)));
        loop {
            let decision = self.decide();
            tracing::trace!(token = ?self.peek(), at = self.pos, ?decision, depth = self.stack.len());
            match decision {
                Decision::Read(at) => {
                    self.end_above(at);
                    self.read_token();
                }
                Decision::Continue(at, kinds) => {
                    self.end_above(at);
                    self.continue_operand(kinds);
                }
                Decision::End => {
                    if let Some(root) = self.end_top() {
                        return root;
                    }
                }
                Decision::Mend => self.mend(),
            }
        }
    }

    /// Ends the frames above the one at depth `at`.
    fn end_above(&mut self, at: usize) {
        while self.stack.len() > at + 1 {
            self.end_top();
        }
    }
}
