//! The concrete syntax tree as a postorder array of `(kind, token, subtree_size)`, in the style of
//! Carbon's parse tree.
//!
//! Children precede their parent, so a node's subtree is the `size` nodes ending at it. Each node
//! is anchored at one token, and every token of the file anchors exactly one node: together with
//! the trivia side table of [`Tokens`], the tree is lossless. A node whose token is absent from the
//! source (a missing `;`, a missing operand) is anchored before the token where it was expected;
//! the parser reported it when it created the node.

use std::fmt;

use overt_grammar::NodeKind;
use overt_index::{Idx, IndexVec, index};
use overt_span::{ByteRange, Span};

use crate::tokens::{TokenId, Tokens};

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

/// Why a node array is not a well-formed tree over its tokens; see [`Cst::verify`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Malformed {
    /// The subtree size of a node does not end on a boundary between its children's subtrees.
    Size { node: NodeId },
    /// A leaf kind has children.
    LeafWithChildren { node: NodeId },
    /// The array is not exactly one tree rooted at a `File` node.
    Root,
    /// A token anchors more than one node.
    Shared { token: TokenId },
    /// A token anchors no node.
    Unowned { token: TokenId },
    /// The tokens of a subtree are not contiguous.
    Scattered { node: NodeId },
}

/// The tokens found in a finished subtree while verifying.
#[derive(Clone, Copy)]
struct Extent {
    start: usize,
    tokens: Option<(TokenId, TokenId)>,
    count: usize,
}

impl Extent {
    fn cover(&mut self, lo: TokenId, hi: TokenId, count: usize) {
        self.tokens = Some(self.tokens.map_or((lo, hi), |(a, b)| (a.min(lo), b.max(hi))));
        self.count += count;
    }

    fn add(&mut self, token: TokenId) {
        self.cover(token, token, 1);
    }

    fn merge(&mut self, child: Self) {
        if let Some((lo, hi)) = child.tokens {
            self.cover(lo, hi, child.count);
        }
    }

    fn contiguous(&self) -> bool {
        self.tokens.is_none_or(|(lo, hi)| hi.index() - lo.index() + 1 == self.count)
    }
}

impl Cst {
    /// Checks the tree's invariants: subtree sizes nest, leaves have no children, the array is one
    /// tree rooted at `File`, every token anchors exactly one node, and every subtree covers a
    /// contiguous run of tokens. This is the verifier of this representation.
    ///
    /// # Errors
    /// Returns the first violation found.
    pub fn verify(&self, tokens: &Tokens) -> Result<(), Malformed> {
        let mut owned = vec![false; tokens.len()];
        let mut finished: Vec<Extent> = Vec::new();
        for n in self.ids() {
            let size = self.size(n);
            let start = (n.index() + 1).checked_sub(size).filter(|_| size > 0);
            let start = start.ok_or(Malformed::Size { node: n })?;
            if self.kind(n).is_leaf() && size != 1 {
                return Err(Malformed::LeafWithChildren { node: n });
            }
            let mut extent = Extent { start, tokens: None, count: 0 };
            let mut first_child = n.index();
            while let Some(child) = finished.pop_if(|e| e.start >= start) {
                first_child = child.start;
                extent.merge(child);
            }
            if first_child != start {
                return Err(Malformed::Size { node: n });
            }
            if let Anchor::Token(t) = self.anchor(n) {
                let slot = owned.get_mut(t.index()).ok_or(Malformed::Unowned { token: t })?;
                if std::mem::replace(slot, true) {
                    return Err(Malformed::Shared { token: t });
                }
                extent.add(t);
            }
            if !extent.contiguous() {
                return Err(Malformed::Scattered { node: n });
            }
            finished.push(extent);
        }
        if finished.len() != 1 || self.kinds.last() != Some(&NodeKind::File) {
            return Err(Malformed::Root);
        }
        match owned.iter().position(|&o| !o) {
            Some(i) => Err(Malformed::Unowned { token: TokenId::new(i) }),
            None => Ok(()),
        }
    }
}

/// See [`Cst::dump`].
pub struct Dump<'a> {
    cst: &'a Cst,
    tokens: &'a Tokens,
    source: &'a str,
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

/// Where a node's children begin: the array length before the first of them was pushed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Mark(usize);

/// Accumulates nodes in postorder.
pub(crate) struct Builder {
    cst: Cst,
}

impl Builder {
    pub(crate) fn new(capacity: usize) -> Self {
        let cst = Cst {
            kinds: IndexVec::with_capacity(capacity),
            anchors: IndexVec::with_capacity(capacity),
            sizes: IndexVec::with_capacity(capacity),
        };
        Self { cst }
    }

    pub(crate) fn mark(&self) -> Mark {
        Mark(self.cst.len())
    }

    /// Pushes a node whose children are everything pushed since `mark`.
    pub(crate) fn node(&mut self, kind: NodeKind, anchor: Anchor, mark: Mark) {
        let size = u32::try_from(self.cst.len() - mark.0 + 1).expect("node count fits u32");
        self.cst.kinds.push(kind);
        self.cst.anchors.push(anchor);
        self.cst.sizes.push(size);
    }

    /// The kind of the node pushed last, whose subtree ends the array.
    pub(crate) fn last(&self) -> Option<NodeKind> {
        self.cst.kinds.last().copied()
    }

    /// Whether the nodes pushed since `mark` form a single subtree.
    pub(crate) fn single_since(&self, mark: Mark) -> bool {
        self.cst.sizes.last().is_some_and(|&size| size as usize == self.cst.len() - mark.0)
    }

    pub(crate) fn finish(self) -> Cst {
        self.cst
    }
}
