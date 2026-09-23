//! The indented text form of a tree, for `overt dump cst` and snapshots.

use std::fmt;

use overt_span::ByteRange;

use super::{Anchor, Cst, NodeId};
use crate::tokens::Tokens;

/// See [`Cst::dump`].
pub struct Dump<'a> {
    pub(super) cst: &'a Cst,
    pub(super) tokens: &'a Tokens,
    pub(super) source: &'a str,
}

/// Indentation per tree level in dumps.
const DUMP_INDENT: usize = 2;

impl Dump<'_> {
    fn node(&self, f: &mut fmt::Formatter<'_>, n: NodeId, depth: usize) -> fmt::Result {
        let indent = depth * DUMP_INDENT;
        write!(f, "{:indent$}{:?} ", "", self.cst.kind(n))?;
        match self.cst.anchor(n) {
            Anchor::Token(t) => {
                let range = self.tokens.range(t);
                let ByteRange { lo, hi } = range;
                writeln!(f, "{:?} {lo}..{hi} {:?}", self.tokens.kind(t), range.of(self.source))?;
            }
            Anchor::Missing(t) => writeln!(f, "missing at {}", self.tokens.range(t).lo)?,
        }
        self.cst.children(n).try_for_each(|c| self.node(f, c, depth + 1))
    }
}

impl fmt::Display for Dump<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.cst.is_empty() {
            return Ok(());
        }
        self.node(f, self.cst.root(), 0)
    }
}
