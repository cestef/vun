//! The control flow graph: each block's successors, in the order its terminator names them, and
//! predecessors, each counted once.

use vun_index::IndexVec;

use crate::entity::Block;
use crate::func::Function;

pub struct Cfg {
    pub succs: IndexVec<Block, Vec<Block>>,
    pub preds: IndexVec<Block, Vec<Block>>,
}

impl Cfg {
    /// The graph of `func`; a jump to a block the function lacks is left out.
    #[must_use]
    pub fn of(func: &Function) -> Self {
        let count = func.blocks.len();
        let mut succs: IndexVec<Block, Vec<Block>> = IndexVec::filled(Vec::new(), count);
        let mut preds: IndexVec<Block, Vec<Block>> = IndexVec::filled(Vec::new(), count);
        for (block, data) in func.blocks.iter() {
            for target in data.term.targets() {
                let to = target.block;
                if func.blocks.get(to).is_some() && !succs[block].contains(&to) {
                    succs[block].push(to);
                    preds[to].push(block);
                }
            }
        }
        Self { succs, preds }
    }
}
