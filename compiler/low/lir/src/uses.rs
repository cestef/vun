//! Where a function's instructions are and how often its values are read: the block holding
//! each instruction, the blocks the entry reaches, and the reads each value has there.

use vun_index::IndexVec;

use crate::entity::{Block, Inst, Value};
use crate::func::Function;

impl Function {
    /// The block holding each instruction; `None` for one no block holds.
    #[must_use]
    pub fn homes(&self) -> IndexVec<Inst, Option<Block>> {
        let mut homes = IndexVec::filled(None, self.insts.len());
        for (block, data) in self.blocks.iter() {
            for &inst in &data.insts {
                homes[inst] = Some(block);
            }
        }
        homes
    }

    /// The blocks control reaches from the entry, as a set indexed by block.
    #[must_use]
    pub fn reached(&self) -> IndexVec<Block, bool> {
        let mut reached = IndexVec::filled(false, self.blocks.len());
        let mut pending: Vec<Block> = self.entry().into_iter().collect();
        while let Some(block) = pending.pop() {
            if !std::mem::replace(&mut reached[block], true) {
                pending.extend(self.blocks[block].term.targets().map(|t| t.block));
            }
        }
        reached
    }

    /// How many reads of each value the reached blocks make.
    #[must_use]
    pub fn uses(&self) -> IndexVec<Value, u32> {
        let mut uses = IndexVec::filled(0, self.values.len());
        let reached = self.reached();
        for (_, data) in self.blocks.iter().filter(|(b, _)| reached[*b]) {
            let insts = data.insts.iter().flat_map(|&i| self.insts[i].kind.operands());
            for value in insts.chain(data.term.operands()) {
                uses[value] += 1;
            }
        }
        uses
    }
}
