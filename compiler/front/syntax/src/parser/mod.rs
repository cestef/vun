//! The parser: the engine reading the productions of `nodes.vun` (`engine`) into the CST.
//!
//! Patterns are read as expressions: they mirror constructors, so `(a, b)`, `.some(x)` and
//! `.{ .x = p }` have one shape in both roles, and later stages decide which role a node plays.
//!
//! Every error is reported once, where it is found, and the tree keeps its shape: absent tokens and
//! children become nodes anchored where they were expected, and skipped tokens `Skipped` leaves.

use vun_diag::DiagCx;

use crate::cst::Builder;
use crate::tokens::Tokens;

mod cursor;
mod engine;
mod report;
#[cfg(test)]
mod tests;

/// How deeply expressions and blocks may nest before the parser gives up on a subtree.
const MAX_NESTING: u32 = 256;

struct Parser<'t, 'c, 'd> {
    tokens: &'t Tokens,
    source: &'t str,
    pos: usize,
    /// How many operands the frames being read hold.
    depth: u32,
    /// The position of the last error, so that one position reports once.
    reported: Option<usize>,
    /// Whether an error was reported since the productions last read a punctuation token in
    /// place: until they do, what goes wrong is the fallout of that error.
    quiet: bool,
    out: Builder,
    /// The frames the engine reads with, and the nodes it read.
    stack: Vec<engine::Frame>,
    tree: engine::Tree,
    cx: &'c mut DiagCx<'d>,
}

impl<'t, 'c, 'd> Parser<'t, 'c, 'd> {
    fn new(tokens: &'t Tokens, source: &'t str, cx: &'c mut DiagCx<'d>) -> Self {
        let out = Builder::new(tokens.len());
        let (stack, tree) = (Vec::new(), engine::Tree::default());
        Parser { tokens, source, pos: 0, depth: 0, reported: None, quiet: false, out, stack, tree, cx }
    }
}
