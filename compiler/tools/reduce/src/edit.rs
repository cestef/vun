//! The edits a reduction tries, each pass's from the tree of the current text, largest first.

use vun_index::Idx;
use vun_span::ByteRange;
use vun_syntax::cst::{Anchor, NodeId};

use crate::parse::Parse;

/// A kind of edit, tried in this order: each removes less than the one before.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Pass {
    /// Runs of a node's children removed, the whole run first, then halves, down to one child.
    Siblings,
    /// A node replaced by one of its children.
    Hoist,
    /// A lexeme removed: a token, a comment or whitespace.
    Lexemes,
}

impl Pass {
    pub(crate) const ALL: [Self; 3] = [Self::Siblings, Self::Hoist, Self::Lexemes];

    pub(crate) fn edits(self, parsed: &Parse, text: &str) -> Vec<Edit> {
        match self {
            Self::Siblings => siblings(parsed),
            Self::Hoist => hoists(parsed, text),
            Self::Lexemes => parsed.tokens.lexemes().map(|l| Edit::remove(l.range)).collect(),
        }
    }
}

/// Text `range` replaced by `with`.
pub(crate) struct Edit {
    range: ByteRange,
    with: String,
}

impl Edit {
    fn remove(range: ByteRange) -> Self {
        Self { range, with: String::new() }
    }

    pub(crate) fn apply(&self, text: &str) -> String {
        let (lo, hi) = (self.range.lo as usize, self.range.hi as usize);
        [&text[..lo], &self.with, &text[hi..]].concat()
    }
}

/// Nodes parents first, so that the largest removals are tried before their parts.
fn preorder(parsed: &Parse) -> impl Iterator<Item = NodeId> {
    (0..parsed.cst.len()).rev().map(NodeId::new)
}

/// The source of `n`, or `None` when it holds no token.
fn range(parsed: &Parse, n: NodeId) -> Option<ByteRange> {
    let span = parsed.cst.span(n, &parsed.tokens).range;
    let present = matches!(parsed.cst.anchor(n), Anchor::Token(_)) || !span.is_empty();
    present.then_some(span)
}

/// For each node, removals of runs of its children: every child, then runs of half as many,
/// down to single children.
fn siblings(parsed: &Parse) -> Vec<Edit> {
    let mut edits = Vec::new();
    for n in preorder(parsed) {
        let children: Vec<ByteRange> = parsed.cst.children(n).filter_map(|c| range(parsed, c)).collect();
        let mut run = children.len();
        while run > 0 {
            for chunk in children.chunks(run) {
                let (first, last) = (chunk[0], chunk[chunk.len() - 1]);
                edits.push(Edit::remove(first.cover(last)));
            }
            run /= 2;
        }
    }
    edits
}

/// Each node replaced by the text of each of its children.
fn hoists(parsed: &Parse, text: &str) -> Vec<Edit> {
    let mut edits = Vec::new();
    for n in preorder(parsed) {
        let Some(whole) = range(parsed, n) else { continue };
        for child in parsed.cst.children(n).filter_map(|c| range(parsed, c)) {
            if child != whole {
                edits.push(Edit { range: whole, with: child.of(text).to_owned() });
            }
        }
    }
    edits
}
