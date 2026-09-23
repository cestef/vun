//! The concrete syntax tree as a postorder array of `(kind, token, subtree_size)`, in the style of
//! Carbon's parse tree.
//!
//! Children precede their parent, so a node's subtree is the `size` nodes ending at it. Each node
//! is anchored at one token, and every token of the file anchors exactly one node: together with
//! the trivia side table of [`Tokens`], the tree is lossless. A node whose token is absent from the
//! source (a missing `;`, a missing operand) is anchored before the token where it was expected;
//! the parser reported it when it created the node.

use overt_grammar::NodeKind;
use overt_index::{Idx, IndexVec, index};
use overt_span::{ByteRange, Span};

use crate::tokens::{TokenId, Tokens};

mod builder;
mod dump;
mod verify;

pub(crate) use builder::Builder;
pub use dump::Dump;
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

    /// The children of `n`, in source order.
    pub fn children(&self, n: NodeId) -> impl Iterator<Item = NodeId> {
        let first = n.index() + 1 - self.size(n);
        let mut children = Vec::new();
        let mut child = n.index();
        while child > first {
            let c = NodeId::new(child - 1);
            children.push(c);
            child -= self.size(c);
        }
        children.into_iter().rev()
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

    /// An indented line per node, for `overt dump cst` and snapshots.
    #[must_use]
    pub fn dump<'a>(&'a self, tokens: &'a Tokens, source: &'a str) -> Dump<'a> {
        Dump { cst: self, tokens, source }
    }
}
