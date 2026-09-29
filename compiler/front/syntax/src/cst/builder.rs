//! Building a tree in postorder.

use vun_grammar::NodeKind;

use crate::tokens::Tokens;
use vun_index::{Idx, IndexVec};

use super::{Anchor, Cst, NodeId};

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

    /// Pushes a node of the first of `kinds` whose production its children spell, each in a field
    /// admitting it with `context` admitted everywhere (`Cst::fits`); an `Error` where none does.
    pub(crate) fn node_of(
        &mut self,
        kinds: &[NodeKind],
        anchor: Anchor,
        mark: Mark,
        context: &[NodeKind],
        tokens: &Tokens,
        source: &str,
    ) -> Option<NodeKind> {
        self.node(NodeKind::Error, anchor, mark);
        let n = NodeId::new(self.cst.len() - 1);
        for &kind in kinds {
            self.cst.kinds[n] = kind;
            if self.cst.fits(n, context, tokens, source) {
                return Some(kind);
            }
        }
        self.cst.kinds[n] = NodeKind::Error;
        None
    }

    /// The kind of the node pushed last, whose subtree ends the array.
    pub(crate) fn last(&self) -> Option<NodeKind> {
        self.cst.kinds.last().copied()
    }

    /// The number of subtrees pushed since `mark`, at the top level, whose root satisfies `keep`.
    pub(crate) fn nodes_since(&self, mark: Mark, keep: impl Fn(NodeKind) -> bool) -> usize {
        let (mut end, mut count) = (self.cst.len(), 0);
        while end > mark.0 {
            let root = end - 1;
            count += usize::from(keep(self.cst.kinds[NodeId::new(root)]));
            end -= self.cst.sizes[NodeId::new(root)] as usize;
        }
        count
    }

    pub(crate) fn finish(self) -> Cst {
        self.cst
    }
}
