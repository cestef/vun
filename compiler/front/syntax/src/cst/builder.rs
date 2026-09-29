//! Building a tree in postorder.

use vun_grammar::NodeKind;
use vun_index::IndexVec;

use super::{Anchor, Cst};

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

    pub(crate) fn finish(self) -> Cst {
        self.cst
    }
}
