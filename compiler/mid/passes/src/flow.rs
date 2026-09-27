//! Forward dataflow over a body's blocks, as the checks run it: a fact at the entry of each block
//! reached, which the block turns into what the blocks it goes to see, joined where paths meet,
//! until nothing changes.

use std::collections::VecDeque;

use vun_index::{BitMatrix, Idx, IndexVec};
use vun_mir::BlockId;

/// What holds at a point, and how what holds on two paths meets.
pub(crate) trait Fact: Clone {
    /// Joins `other` into `self`, giving whether `self` changed.
    fn join(&mut self, other: &Self) -> bool;
}

impl<R: Idx, C: Idx> Fact for BitMatrix<R, C> {
    fn join(&mut self, other: &Self) -> bool {
        self.union(other)
    }
}

/// A forward problem: what a block does to the fact at its entry.
pub(crate) trait Forward {
    type Fact: Fact;

    /// Runs `block` over `fact`, giving the blocks it goes to.
    fn transfer(&mut self, block: BlockId, fact: &mut Self::Fact) -> Vec<BlockId>;

    /// The fact at the entry of each of `blocks` blocks reached from the first, which `entry`
    /// holds at.
    fn solve(&mut self, blocks: usize, entry: Self::Fact) -> IndexVec<BlockId, Option<Self::Fact>> {
        let mut at: IndexVec<BlockId, Option<Self::Fact>> = IndexVec::filled(None, blocks);
        at[BlockId::new(0)] = Some(entry);
        let mut work = VecDeque::from([BlockId::new(0)]);
        while let Some(block) = work.pop_front() {
            let Some(mut fact) = at[block].clone() else { continue };
            for next in self.transfer(block, &mut fact) {
                let changed = match &mut at[next] {
                    Some(there) => there.join(&fact),
                    slot @ None => {
                        *slot = Some(fact.clone());
                        true
                    }
                };
                if changed {
                    work.push_back(next);
                }
            }
        }
        at
    }
}
