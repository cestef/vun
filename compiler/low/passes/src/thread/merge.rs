//! Merging a block into the one jumping to it when that one is its only predecessor: the jump
//! and the block boundary go, the block's parameters are the jump's arguments. A function
//! lowered as an entry jumping to its body becomes one block, which inlining reads as small.

use vun_index::IndexVec;

use crate::replaced::Replaced;
use vun_lir::analysis::Cfg;
use vun_lir::entity::Block;
use vun_lir::func::Function;
use vun_lir::inst::Terminator;

pub(crate) trait Merge {
    /// Merges every block into its only predecessor ending in a jump to it, returning how many
    /// were; the blocks merged are left empty and unreached.
    fn merge(&mut self) -> usize;
}

impl Merge for Function {
    fn merge(&mut self) -> usize {
        let preds = Cfg::of(self).preds;
        let entry = self.entry();
        // Where each block's code went: itself, or the block it was merged into.
        let mut into: IndexVec<Block, Block> = self.blocks.iter().map(|(b, _)| b).collect::<Vec<_>>().into();
        let mut replaced = Replaced::default();
        let mut count = 0;
        let blocks: Vec<Block> = self.blocks.iter().map(|(b, _)| b).collect();
        for at in blocks {
            if into[at] != at {
                continue;
            }
            while let Terminator::Jump(target) = &self.blocks[at].term {
                let next = target.block;
                let only = matches!(preds[next].as_slice(), [p] if into[*p] == at);
                if next == at || Some(next) == entry || !only {
                    break;
                }
                let args = target.args.clone();
                let merged = &mut self.blocks[next];
                // A block passing its own parameter to itself is unreached, and names it for nothing.
                for (&param, arg) in merged.params.iter().zip(args).filter(|&(&p, a)| p != a) {
                    replaced.insert(param, arg);
                }
                merged.params.clear();
                let insts = std::mem::take(&mut merged.insts);
                let term = std::mem::replace(&mut merged.term, Terminator::Unreachable);
                self.blocks[at].insts.extend(insts);
                self.blocks[at].term = term;
                into[next] = at;
                count += 1;
            }
        }
        if count > 0 {
            for (_, data) in self.blocks.iter_mut() {
                for &inst in &data.insts {
                    self.insts[inst].kind.map_values(|v| replaced.resolve(v));
                }
                data.term.map_values(|v| replaced.resolve(v));
            }
            self.compact();
        }
        count
    }
}
