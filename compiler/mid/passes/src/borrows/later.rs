//! Where a borrow in conflict is used later: the first read, going forward from the conflict, of
//! a local holding it, which is what keeps the loan live there.

use std::collections::VecDeque;

use vun_index::{BitSet, IndexVec};
use vun_mir::{BlockId, Local, Operand};
use vun_origin::OriginId;

use super::Checker;

impl Checker<'_> {
    /// The first place after statement `index` of `block` that reads one of `holders`, on the
    /// paths from there in breadth-first order.
    pub(super) fn later_use(
        &self,
        block: BlockId,
        index: usize,
        holders: &BitSet<Local>,
    ) -> Option<OriginId> {
        let blocks = &self.body.blocks;
        let mut seen: IndexVec<BlockId, bool> = IndexVec::filled(false, blocks.len());
        let mut work = VecDeque::from([(block, index + 1)]);
        while let Some((at, from)) = work.pop_front() {
            let b = &blocks[at];
            for stmt in b.stmts.iter().skip(from) {
                let reads = self.accesses(stmt).into_iter().any(|(place, _)| holders.contains(place.local));
                let through = !stmt.place.projection.is_empty() && holders.contains(stmt.place.local);
                if reads || through {
                    return Some(stmt.origin);
                }
            }
            let term = b.term.kind.operands().into_iter().any(|o| match o {
                Operand::Place(place) => holders.contains(place.local),
                Operand::Const(_) => false,
            });
            if term {
                return Some(b.term.origin);
            }
            for next in b.term.kind.successors() {
                if !std::mem::replace(&mut seen[next], true) {
                    work.push_back((next, 0));
                }
            }
        }
        None
    }
}
