//! The concrete syntax tree as a postorder array of `(kind, token, subtree_size)`, in the style of
//! Carbon's parse tree.
//!
//! Children precede their parent, so a node's subtree is the `size` nodes ending at it. Each node
//! is anchored at one token, and every token of the file anchors exactly one node: together with
//! the trivia side table of [`Tokens`], the tree is lossless. A node whose token is absent from the
//! source (a missing `;`, a missing operand) is anchored before the token where it was expected;
//! the parser reported it when it created the node.

use vun_grammar::NodeKind;
use vun_index::{Idx, IndexVec, index};
use vun_span::{ByteRange, Span};

use crate::tokens::{TokenId, Tokens};

mod builder;
mod dump;
mod fields;
mod production;
mod verify;

pub(crate) use builder::Builder;
pub use dump::Dump;
pub use fields::Fields;
pub use verify::Malformed;

index! {
    pub struct NodeId;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    Token(TokenId),
    /// Absent from the source; it would stand right before this token.
    Missing(TokenId),
}

impl Anchor {
    #[must_use]
    pub fn token(self) -> TokenId {
        match self {
            Self::Token(t) | Self::Missing(t) => t,
        }
    }
}

pub struct Cst {
    kinds: IndexVec<NodeId, NodeKind>,
    anchors: IndexVec<NodeId, Anchor>,
    sizes: IndexVec<NodeId, u32>,
}

impl Cst {
    #[must_use]
    pub fn len(&self) -> usize {
        self.kinds.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.kinds.is_empty()
    }

    #[must_use]
    pub fn kind(&self, n: NodeId) -> NodeKind {
        self.kinds[n]
    }

    #[must_use]
    pub fn anchor(&self, n: NodeId) -> Anchor {
        self.anchors[n]
    }

    /// Number of nodes in the subtree of `n`, including `n`.
    #[must_use]
    pub fn size(&self, n: NodeId) -> usize {
        self.sizes[n] as usize
    }

    /// The `File` node, last in postorder.
    ///
    /// # Panics
    /// Panics on an empty tree; the parser always produces a root.
    #[must_use]
    pub fn root(&self) -> NodeId {
        NodeId::new(self.len().checked_sub(1).expect("a parsed tree has a root"))
    }

    pub fn ids(&self) -> impl Iterator<Item = NodeId> {
        (0..self.len()).map(NodeId::new)
    }

    /// The most nodes on a path from the root to a leaf, found without recursion: in reverse
    /// postorder every node follows its ancestors, which are those whose subtree holds it.
    #[must_use]
    pub fn depth(&self) -> usize {
        let mut starts: Vec<usize> = Vec::new();
        let mut deepest = 0;
        for n in (0..self.len()).rev().map(NodeId::new) {
            while starts.last().is_some_and(|&start| start > n.index()) {
                starts.pop();
            }
            starts.push(n.index() + 1 - self.size(n));
            deepest = deepest.max(starts.len());
        }
        deepest
    }

    /// The children of `n`, last first. Postorder yields them in this order without allocating.
    pub fn children_rev(&self, n: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        let first = n.index() + 1 - self.size(n);
        let mut end = n.index();
        std::iter::from_fn(move || {
            (end > first).then(|| {
                let child = NodeId::new(end - 1);
                end -= self.size(child);
                child
            })
        })
    }

    /// The children of `n`, in source order.
    #[must_use]
    pub fn children(&self, n: NodeId) -> impl DoubleEndedIterator<Item = NodeId> {
        let mut children: Vec<_> = self.children_rev(n).collect();
        children.reverse();
        children.into_iter()
    }

    /// The source covered by the subtree of `n`; empty at the anchor when it holds no token.
    #[must_use]
    pub fn span(&self, n: NodeId, tokens: &Tokens) -> Span {
        let subtree = n.index() + 1 - self.size(n)..=n.index();
        let present = subtree.filter_map(|i| match self.anchors[NodeId::new(i)] {
            Anchor::Token(t) => Some(tokens.range(t)),
            Anchor::Missing(_) => None,
        });
        let range = present.reduce(ByteRange::cover).unwrap_or_else(|| {
            let lo = tokens.range(self.anchors[n].token()).lo;
            ByteRange::new(lo, lo)
        });
        Span { file: tokens.file(), range }
    }

    /// An indented line per node, for `vun debug cst` and snapshots.
    #[must_use]
    pub fn dump<'a>(&'a self, tokens: &'a Tokens, source: &'a str) -> Dump<'a> {
        Dump { cst: self, tokens, source }
    }
}
