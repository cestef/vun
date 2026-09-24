//! Dominators: which blocks every path from the entry to a block goes through. A block no path
//! reaches is dominated by every block, so nothing in it is rejected for its values.

use vun_index::{Idx, IndexVec};

use crate::analysis::Cfg;
use crate::entity::Block;
use crate::func::Function;

pub(super) struct Dominators(IndexVec<Block, Vec<bool>>);

impl Dominators {
    /// The dominators of every block of `func`, iterated to the fixed point from `entry`.
    pub(super) fn of(func: &Function, entry: Block) -> Self {
        let count = func.blocks.len();
        let preds = Cfg::of(func).preds;
        let mut dom: IndexVec<Block, Vec<bool>> = IndexVec::filled(vec![true; count], count);
        dom[entry] = (0..count).map(|i| i == entry.index()).collect();
        let mut changed = true;
        while changed {
            changed = false;
            for (block, _) in func.blocks.iter().filter(|&(b, _)| b != entry) {
                let mut next = vec![true; count];
                for &pred in &preds[block] {
                    for (bit, &d) in next.iter_mut().zip(&dom[pred]) {
                        *bit &= d;
                    }
                }
                next[block.index()] = true;
                if next != dom[block] {
                    dom[block] = next;
                    changed = true;
                }
            }
        }
        Self(dom)
    }

    /// Whether every path from the entry to `block` goes through `def`.
    pub(super) fn dominates(&self, def: Block, block: Block) -> bool {
        self.0.get(block).and_then(|d| d.get(def.index())).copied().unwrap_or(false)
    }
}
