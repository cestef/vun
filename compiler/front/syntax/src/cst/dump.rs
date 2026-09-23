//! The indented text form of a tree, for `overt dump cst` and snapshots.

use std::fmt;

use overt_grammar::NodeField;
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
    fn node(
        &self,
        f: &mut fmt::Formatter<'_>,
        n: NodeId,
        depth: usize,
        field: Option<NodeField>,
    ) -> fmt::Result {
        let indent = depth * DUMP_INDENT;
        write!(f, "{:indent$}", "")?;
        if let Some(field) = field {
            write!(f, "{}: ", field.name())?;
        }
        write!(f, "{:?} ", self.cst.kind(n))?;
        match self.cst.anchor(n) {
            Anchor::Token(t) => {
                let range = self.tokens.range(t);
                let ByteRange { lo, hi } = range;
                writeln!(f, "{:?} {lo}..{hi} {:?}", self.tokens.kind(t), range.of(self.source))?;
            }
            Anchor::Missing(t) => writeln!(f, "missing at {}", self.tokens.range(t).lo)?,
        }
        let fields = self.cst.fields(n).unwrap_or_default();
        let mut assigned = fields.assigned().peekable();
        self.cst.children(n).try_for_each(|c| {
            // Children and assignments are both in source order; punctuation has no field.
            let field = assigned.next_if(|&(node, _)| node == c).map(|(_, field)| field);
            self.node(f, c, depth + 1, field)
        })
    }
}

impl fmt::Display for Dump<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.cst.is_empty() {
            return Ok(());
        }
        self.node(f, self.cst.root(), 0, None)
    }
}
