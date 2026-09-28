//! Dataflow over a body's blocks, as the checks run it, until nothing changes. Forward: a fact at
//! the entry of each block reached, which the block turns into what the blocks it goes to see,
//! joined where paths meet. Backward: a fact at the entry of each block, from those at the
//! entries of the blocks it goes to, joined, and what the block does before them.

use std::collections::VecDeque;

use vun_index::{BitMatrix, BitSet, Idx, IndexVec};
use vun_mir::BlockId;

/// What holds at a point, and how what holds on two paths meets.
pub(crate) trait Fact: Clone {
    /// Joins `other` into `self`, giving whether `self` changed.
    fn join(&mut self, other: &Self) -> bool;
}

impl<I: Idx> Fact for BitSet<I> {
    fn join(&mut self, other: &Self) -> bool {
        self.union(other)
    }
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

/// A backward problem: the blocks each block goes to, and what a block does to the fact at its
/// exit, which gives the fact at its entry.
pub(crate) trait Backward {
    type Fact: Fact + Default + PartialEq;

    /// The blocks `block` goes to.
    fn successors(&self, block: BlockId) -> Vec<BlockId>;

    /// Turns the fact at the exit of `block` into the fact at its entry.
    fn back(&mut self, block: BlockId, fact: &mut Self::Fact);

    /// The fact at the exit of `block`: those at the entries of the blocks it goes to, joined.
    fn exit(&self, block: BlockId, entry: &IndexVec<BlockId, Self::Fact>) -> Self::Fact {
        let mut fact = Self::Fact::default();
        for next in self.successors(block) {
            fact.join(&entry[next]);
        }
        fact
    }

    /// The fact at the entry of each of `blocks` blocks.
    fn solve_backward(&mut self, blocks: usize) -> IndexVec<BlockId, Self::Fact> {
        let mut predecessors: IndexVec<BlockId, Vec<BlockId>> = IndexVec::filled(Vec::new(), blocks);
        for block in (0..blocks).map(BlockId::new) {
            for next in self.successors(block) {
                predecessors[next].push(block);
            }
        }
        let mut entry: IndexVec<BlockId, Self::Fact> = IndexVec::filled(Self::Fact::default(), blocks);
        let mut work: VecDeque<BlockId> = (0..blocks).map(BlockId::new).collect();
        while let Some(block) = work.pop_front() {
            let mut fact = self.exit(block, &entry);
            self.back(block, &mut fact);
            if fact != entry[block] {
                entry[block] = fact;
                work.extend(predecessors[block].iter().copied());
            }
        }
        entry
    }
}
