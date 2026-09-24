//! Building a tree in postorder.

use vun_grammar::NodeKind;
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

    /// The kind of the node pushed last, whose subtree ends the array.
    pub(crate) fn last(&self) -> Option<NodeKind> {
        self.cst.kinds.last().copied()
    }

    /// Whether the nodes pushed since `mark` form a single subtree.
    pub(crate) fn single_since(&self, mark: Mark) -> bool {
        self.cst.sizes.last().is_some_and(|&size| size as usize == self.cst.len() - mark.0)
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
