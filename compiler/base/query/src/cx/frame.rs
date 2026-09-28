//! A computation on a thread's stack.

use crate::memo::Dep;
use crate::table::{Slot, TableId};

/// A query being computed, and each memo it read.
pub(super) struct Frame {
    pub(super) table: TableId,
    pub(super) slot: Slot,
    pub(super) reads: Vec<Dep>,
    /// The lowest frame on the path whose cycle value this computation read, itself or through
    /// what it read, by its depth on the path (the lenders' frames first, see `Cx::path`).
    pub(super) head: Option<usize>,
}

/// Whether a value computed holds beyond the computation that asked for it: one computed from
/// the cycle value of a query still being computed holds only for that computation, since the
/// query's own value may differ, and which query of a cycle is reached first must not change
/// what the others are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Standing {
    Final,
    Provisional,
}
