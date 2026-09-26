//! Threading jumps (spec 13, section 3): a block deciding where to go from a parameter, by
//! branching or switching on it or on its comparison with a constant, as the join of
//! `if c { true } else { false }` or the head of a loop over a state does, is jumped over by
//! each predecessor passing it a constant, which goes where the block would send that constant;
//! where blocks after it read its parameters, they go through slots that promotion makes values
//! again. A block that only jumps is jumped over by its predecessors. Blocks no longer reached
//! stay until they are swept.

mod decide;
mod demote;
mod merge;
#[cfg(test)]
mod tests;

use vun_hash::Set;

use crate::analysis::Cfg;
use vun_index::IndexVec;

use crate::entity::{Block, Inst, Value};
use crate::func::{Def, Function};
use crate::inst::{InstKind, Target, Terminator};

impl Function {
    /// Threads jumps through blocks deciding on a constant and blocks only jumping, until none
    /// is left, returning how many jumps were moved.
    pub fn thread(&mut self, pointer_bits: u32) -> usize {
        let mut moved = 0;
        loop {
            let now = self.thread_constants(pointer_bits) + self.forward() + self.merge();
            if now == 0 {
                return moved;
            }
            moved += now;
        }
    }

    /// Moves each jump passing a constant to a block deciding on that parameter (see
    /// `Function::decides`) to where the block goes for that constant. When blocks after it read
    /// its parameters, they are demoted to slots first (`Function::demote_params`), every jump
    /// to the block being a `Jump`, and the pass stops there for promotion to rebuild them.
    fn thread_constants(&mut self, pointer_bits: u32) -> usize {
        let preds = Cfg::of(self).preds;
        let homes = self.homes();
        let outside = self.read_outside(&homes);
        let entry = self.entry();
        let mut count = 0;
        let joins: Vec<Block> = self.blocks.iter().map(|(b, _)| b).filter(|&b| Some(b) != entry).collect();
        for join in joins {
            let Some(decided) = self.decides(join, &homes) else { continue };
            let data = &self.blocks[join];
            let (term, params) = (data.term.clone(), data.params.clone());
            let passing: Vec<(Block, Vec<Value>, Target)> = preds[join]
                .iter()
                .filter_map(|&pred| {
                    let Terminator::Jump(to) = &self.blocks[pred].term else { return None };
                    let (_, bits) = self.constant(*to.args.get(decided.param)?)?;
                    let taken = decided.taken(&term, bits)?;
                    (to.block == join && taken.block != join).then(|| (pred, to.args.clone(), taken.clone()))
                })
                .collect();
            let computed = data.insts.iter().filter_map(|&i| self.insts[i].result);
            if passing.is_empty() || computed.into_iter().any(|v| outside.contains(&v)) {
                continue;
            }
            if !params.iter().any(|p| outside.contains(p)) {
                for (pred, args, taken) in passing {
                    let args = taken
                        .args
                        .iter()
                        .map(|v| params.iter().position(|p| p == v).map_or(*v, |at| args[at]));
                    self.blocks[pred].term =
                        Terminator::Jump(Target { block: taken.block, args: args.collect() });
                    count += 1;
                }
                continue;
            }
            if !preds[join].iter().all(|&p| matches!(self.blocks[p].term, Terminator::Jump(_))) {
                continue;
            }
            let slots = self.demote_params(join, pointer_bits);
            for (pred, _, taken) in &passing {
                let mut insts = std::mem::take(&mut self.blocks[*pred].insts);
                let origin = self.blocks[*pred].origin;
                let loaded = self.load_params(&slots, taken.args.iter().copied(), origin, &mut insts);
                let args = taken.args.iter().map(|v| loaded.get(v).copied().unwrap_or(*v)).collect();
                self.blocks[*pred].insts = insts;
                self.blocks[*pred].term = Terminator::Jump(Target { block: taken.block, args });
            }
            return count + passing.len();
        }
        count
    }

    /// Moves each jump to a block that holds only constants read nowhere else and jumps
    /// without arguments to where that block jumps.
    fn forward(&mut self) -> usize {
        let preds = Cfg::of(self).preds;
        let homes = self.homes();
        let outside = self.read_outside(&homes);
        let mut count = 0;
        let entry = self.entry();
        let blocks: Vec<Block> = self.blocks.iter().map(|(b, _)| b).filter(|&b| Some(b) != entry).collect();
        for through in blocks {
            let data = &self.blocks[through];
            let Terminator::Jump(onward) = &data.term else { continue };
            let only_constants = data.insts.iter().all(|&i| {
                let inst = &self.insts[i];
                matches!(inst.kind, InstKind::Const { .. })
                    && inst.result.is_none_or(|v| !outside.contains(&v))
            });
            let local = |v: Value| matches!(self.values[v].def, Def::Inst(i) if homes[i] == Some(through));
            if !data.params.is_empty()
                || !only_constants
                || onward.block == through
                || onward.args.iter().any(|&v| local(v))
            {
                continue;
            }
            let onward = onward.clone();
            for &pred in &preds[through] {
                let term = &self.blocks[pred].term;
                // One edge per pair: a jump already going there with other arguments stays.
                if term.targets().any(|t| t.block == onward.block && t.args != onward.args) {
                    continue;
                }
                for target in self.blocks[pred].term.targets_mut().filter(|t| t.block == through) {
                    *target = onward.clone();
                    count += 1;
                }
            }
        }
        count
    }

    /// The values read outside the block defining them: what a block jumped over must not
    /// define, since the blocks after it would read what no longer is.
    fn read_outside(&self, homes: &IndexVec<Inst, Option<Block>>) -> Set<Value> {
        let mut outside = Set::default();
        for (block, data) in self.blocks.iter() {
            let here = |v: Value| match self.values[v].def {
                Def::Param(b, _) => b == block,
                Def::Inst(i) => homes[i] == Some(block),
            };
            let reads =
                data.insts.iter().flat_map(|&i| self.insts[i].kind.operands()).chain(data.term.operands());
            outside.extend(reads.filter(|&v| !here(v)));
        }
        outside
    }

    /// The position of `value` among the parameters of `block`, if it is one.
    fn param(&self, block: Block, value: Value) -> Option<usize> {
        self.blocks[block].params.iter().position(|&p| p == value)
    }
}
