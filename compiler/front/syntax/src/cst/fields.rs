//! Named children: a branch's children, punctuation left out, assigned to the slots its kind
//! declares in `grammar.vun` (`NodeKind::slots`).
//!
//! Assignment runs left to right and backtracks: `many` slots take as many nodes as they can and
//! `optional` slots take one when they can, giving back when a later slot would go unfilled. A
//! node that a later slot is restricted to is left to that slot first. A slot
//! restricted to a kind also accepts `Error`, the placeholder the parser leaves for a missing node.

use vun_grammar::{Count, NodeField, NodeKind, NodeSlot};

use super::{Cst, NodeId};

/// The children of one node, by field. See [`Cst::fields`]. The default has no fields.
#[derive(Default)]
pub struct Fields {
    slots: &'static [NodeSlot],
    nodes: Vec<NodeId>,
    /// For each slot, the range of `nodes` it holds.
    bounds: Vec<(usize, usize)>,
}

impl Fields {
    /// The nodes of `field`, empty if the kind has no such field.
    #[must_use]
    pub fn get(&self, field: NodeField) -> &[NodeId] {
        self.slots
            .iter()
            .position(|s| s.field == field)
            .map_or(&[], |i| &self.nodes[self.bounds[i].0..self.bounds[i].1])
    }

    /// Every child with its field, in source order.
    pub fn assigned(&self) -> impl Iterator<Item = (NodeId, NodeField)> + '_ {
        self.slots
            .iter()
            .zip(&self.bounds)
            .flat_map(|(slot, &(lo, hi))| self.nodes[lo..hi].iter().map(|&n| (n, slot.field)))
    }

    /// The node of a `one` or `optional` field, if present.
    #[must_use]
    pub fn one(&self, field: NodeField) -> Option<NodeId> {
        self.get(field).first().copied()
    }
}

impl Cst {
    /// The children of `n` assigned to its kind's slots, or `None` if they do not fit them, which
    /// the verifier rules out for every tree the parser builds.
    #[must_use]
    pub fn fields(&self, n: NodeId) -> Option<Fields> {
        let slots = self.kind(n).slots();
        let nodes: Vec<NodeId> = self.children(n).filter(|&c| self.kind(c) != NodeKind::Punct).collect();
        let mut bounds = Vec::with_capacity(slots.len());
        self.assign(slots, &nodes, 0, &mut bounds).then_some(Fields { slots, nodes, bounds })
    }

    fn assign(
        &self,
        slots: &[NodeSlot],
        nodes: &[NodeId],
        at: usize,
        bounds: &mut Vec<(usize, usize)>,
    ) -> bool {
        let Some((slot, rest)) = slots.split_first() else { return at == nodes.len() };
        let fits = nodes[at..].iter().take_while(|&&c| self.accepts(*slot, c)).count();
        let (fewest, most) = match slot.count {
            Count::One => (1, 1),
            Count::Optional => (0, 1),
            Count::Many => (0, fits),
        };
        let most = most.min(fits);
        // Prefer leaving a node to a later slot restricted to its kind: `returns` after `params`.
        let claimed = nodes[at..at + most].iter().position(|&c| self.claimed(rest, c)).unwrap_or(most);
        let preferred = (fewest..=claimed.max(fewest).min(most)).rev();
        let fallback = (claimed.max(fewest) + 1..=most).rev();
        for taken in preferred.chain(fallback) {
            bounds.push((at, at + taken));
            if self.assign(rest, nodes, at + taken, bounds) {
                return true;
            }
            bounds.pop();
        }
        false
    }

    /// Whether a slot in `rest` is restricted to the kind of `n`.
    fn claimed(&self, rest: &[NodeSlot], n: NodeId) -> bool {
        rest.iter().any(|s| s.kind == Some(self.kind(n)))
    }

    fn accepts(&self, slot: NodeSlot, n: NodeId) -> bool {
        let kind = self.kind(n);
        slot.kind.is_none_or(|k| k == kind || kind == NodeKind::Error)
    }
}
