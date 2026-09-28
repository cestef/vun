//! Sweeping what a function no longer needs after its other passes: the blocks control never
//! reaches go, then every parameter only passed back to itself or passed one value, and every
//! instruction that only defines a value nothing reads, with what only they read. Lowering
//! guarantees each operation's domain (`facts`), so dropping one never drops a trap.

use vun_hash::{Map, Set};
use vun_index::{Idx, IndexVec};

use crate::replaced::Replaced;
use vun_lir::entity::{Block, Inst, Value};
use vun_lir::func::{Def, Function};

pub trait Sweep {
    /// Drops the unreached blocks, then until none is left the parameters nothing reads but the
    /// jumps passing them back, and the instructions that do nothing but define an unread value
    /// (`InstKind::acts`); returns how many blocks, parameters and instructions went.
    fn sweep(&mut self) -> usize;
}

impl Sweep for Function {
    fn sweep(&mut self) -> usize {
        let mut count = self.drop_unreached();
        loop {
            let now = self.drop_unread_params() + self.drop_passed_params() + self.drop_unread_insts();
            if now == 0 {
                break;
            }
            count += now;
        }
        if count > 0 {
            self.compact();
        }
        count
    }
}

trait Steps {
    fn drop_unread_insts(&mut self) -> usize;

    /// Drops each parameter of a block other than the entry read only by jumps to that block
    /// passing it in its own place, as a loop passes on what it never uses, with the arguments
    /// passed for it.
    fn drop_unread_params(&mut self) -> usize;

    /// Replaces each parameter of a block other than the entry to which every jump passes one
    /// value, or the parameter itself, by that value, dropping it with the arguments passed.
    fn drop_passed_params(&mut self) -> usize;

    /// Drops the blocks control never reaches, renumbering the rest in their order, returning
    /// how many went. What they defined goes when the function is compacted.
    fn drop_unreached(&mut self) -> usize;
}

impl Steps for Function {
    fn drop_unread_insts(&mut self) -> usize {
        let mut uses = self.uses();
        let unread = |f: &Self, inst: Inst, uses: &IndexVec<_, u32>| {
            let data = &f.insts[inst];
            !data.kind.acts() && data.result.is_none_or(|v| uses[v] == 0)
        };
        let mut pending: Vec<Inst> = self
            .blocks
            .iter()
            .flat_map(|(_, b)| b.insts.iter().copied())
            .filter(|&i| unread(self, i, &uses))
            .collect();
        let mut dropped = Set::default();
        while let Some(inst) = pending.pop() {
            if !dropped.insert(inst) {
                continue;
            }
            for value in self.insts[inst].kind.operands() {
                uses[value] -= 1;
                if let Def::Inst(def) = self.values[value].def
                    && uses[value] == 0
                    && unread(self, def, &uses)
                {
                    pending.push(def);
                }
            }
        }
        for (_, data) in self.blocks.iter_mut() {
            data.insts.retain(|inst| !dropped.contains(inst));
        }
        dropped.len()
    }

    fn drop_unread_params(&mut self) -> usize {
        let uses = self.uses();
        let mut passed_back: IndexVec<Value, u32> = IndexVec::filled(0, self.values.len());
        for (_, data) in self.blocks.iter() {
            for target in data.term.targets() {
                let params = &self.blocks[target.block].params;
                for (&arg, _) in target.args.iter().zip(params).filter(|(a, p)| a == p) {
                    passed_back[arg] += 1;
                }
            }
        }
        let entry = self.entry();
        let unread: IndexVec<Block, Vec<bool>> = self
            .blocks
            .iter()
            .map(|(block, data)| {
                data.params.iter().map(|&p| Some(block) != entry && uses[p] == passed_back[p]).collect()
            })
            .collect::<Vec<_>>()
            .into();
        let count = unread.iter().map(|(_, u)| u.iter().filter(|&&u| u).count()).sum();
        if count > 0 {
            for (block, data) in self.blocks.iter_mut() {
                let mut unread_param = unread[block].iter();
                data.params.retain(|_| !unread_param.next().copied().unwrap_or(false));
                for target in data.term.targets_mut() {
                    let mut unread_arg = unread[target.block].iter();
                    target.args.retain(|_| !unread_arg.next().copied().unwrap_or(false));
                }
            }
        }
        count
    }

    fn drop_passed_params(&mut self) -> usize {
        let entry = self.entry();
        let mut passed: Map<(Block, usize), Value> = Map::default();
        let mut several: Set<(Block, usize)> = Set::default();
        for (_, data) in self.blocks.iter() {
            for target in data.term.targets() {
                let params = &self.blocks[target.block].params;
                for (at, (&arg, &param)) in target.args.iter().zip(params).enumerate() {
                    let key = (target.block, at);
                    if arg != param && *passed.entry(key).or_insert(arg) != arg {
                        several.insert(key);
                    }
                }
            }
        }
        let mut replaced = Replaced::default();
        let mut dropped: Map<Block, Vec<bool>> = Map::default();
        for ((block, at), value) in passed {
            if Some(block) == entry || several.contains(&(block, at)) {
                continue;
            }
            replaced.insert(self.blocks[block].params[at], value);
            dropped.entry(block).or_insert_with(|| vec![false; self.blocks[block].params.len()])[at] = true;
        }
        if dropped.is_empty() {
            return 0;
        }
        for (block, data) in self.blocks.iter_mut() {
            if let Some(gone) = dropped.get(&block) {
                let mut gone = gone.iter();
                data.params.retain(|_| !gone.next().copied().unwrap_or(false));
            }
            for target in data.term.targets_mut() {
                if let Some(gone) = dropped.get(&target.block) {
                    let mut gone = gone.iter();
                    target.args.retain(|_| !gone.next().copied().unwrap_or(false));
                }
            }
            for &inst in &data.insts {
                self.insts[inst].kind.map_values(|v| replaced.resolve(v));
            }
            data.term.map_values(|v| replaced.resolve(v));
        }
        replaced.len()
    }

    fn drop_unreached(&mut self) -> usize {
        let reached = self.reached();
        let unreached = reached.iter().filter(|(_, r)| !**r).count();
        if unreached == 0 {
            return 0;
        }
        let mut renumbered: IndexVec<Block, Option<Block>> = IndexVec::filled(None, self.blocks.len());
        let mut blocks = IndexVec::with_capacity(self.blocks.len() - unreached);
        for (block, data) in std::mem::take(&mut self.blocks).into_vec().into_iter().enumerate() {
            let block = Block::new(block);
            if reached[block] {
                renumbered[block] = Some(blocks.push(data));
            }
        }
        for (_, data) in blocks.iter_mut() {
            for target in data.term.targets_mut() {
                target.block = renumbered[target.block].expect("a reached block jumps to reached ones");
            }
        }
        self.blocks = blocks;
        unreached
    }
}
