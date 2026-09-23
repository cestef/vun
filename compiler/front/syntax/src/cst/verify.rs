//! The verifier of the tree: sizes nest, leaves are leaves, tokens are owned once and contiguously.

use overt_grammar::NodeKind;
use overt_index::Idx;

use super::{Anchor, Cst, NodeId};
use crate::tokens::{TokenId, Tokens};

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
