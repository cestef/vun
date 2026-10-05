//! The construction: each load becomes the definition reaching it, a block where several reach
//! gets a parameter, filled by each predecessor with the definition at its end, and parameters
//! every predecessor fills with one value (or with themselves) are that value.

use vun_hash::{Map, Set};

use vun_index::IndexVec;

use super::Uses;
use crate::replaced::Replaced;
use vun_lir::analysis::{Cfg, Dominators};
use vun_lir::entity::{Block, Inst, Slot, Value};
use vun_lir::func::{Def, Function, InstData, ValueData};
use vun_lir::inst::InstKind;

/// What a promoted instruction does to its slot.
enum Step {
    Define(Value),
    /// Reads it into the load's value.
    Read(Value),
    Address,
}

pub(super) struct Promotion<'f> {
    func: &'f mut Function,
    uses: Uses,
    preds: IndexVec<Block, Vec<Block>>,
    /// The last store of each slot in each block.
    ends: Map<(Block, Slot), Value>,
    /// The definition of each slot reaching the start of each block, once asked for.
    starts: Map<(Block, Slot), Value>,
    /// The parameters added to each block, in order, with their slots.
    params: Map<Block, Vec<(Slot, Value)>>,
    /// Parameters whose predecessors' definitions are not asked for yet.
    pending: Vec<(Block, Slot, Value)>,
    /// What each predecessor passes each parameter.
    incoming: Map<Value, Vec<(Block, Value)>>,
    replaced: Replaced,
    /// The value of a slot read before any store, a constant at the entry, once asked for.
    unset: Map<Slot, (Inst, Value)>,
    dominators: Dominators,
    /// The slots stored once, with the block and the value: that value reaches every block the
    /// store's block dominates.
    sole: Map<Slot, (Block, Value)>,
    /// Blocks looked through for definitions, and parameters made: what the pass cost.
    steps: usize,
    made: Map<Slot, usize>,
    /// How many stores each slot has.
    stored: Map<Slot, usize>,
}

impl<'f> Promotion<'f> {
    pub(super) fn new(func: &'f mut Function, uses: Uses) -> Self {
        let cfg = Cfg::of(func);
        let dominators = Dominators::of(func, &cfg);
        let preds = cfg.preds;
        Self {
            func,
            uses,
            preds,
            ends: Map::default(),
            starts: Map::default(),
            params: Map::default(),
            pending: Vec::new(),
            incoming: Map::default(),
            replaced: Replaced::default(),
            unset: Map::default(),
            dominators,
            sole: Map::default(),
            steps: 0,
            made: Map::default(),
            stored: Map::default(),
        }
    }

    /// The slot a load or store of `inst` reaches, if it is promoted.
    fn promoted(&self, kind: &InstKind) -> Option<Slot> {
        let addr = match kind {
            InstKind::Load { addr, .. } | InstKind::Store { addr, .. } => addr,
            InstKind::SlotAddr(slot) => return self.uses.promoted.contains_key(slot).then_some(*slot),
            _ => return None,
        };
        self.uses.addresses.get(addr).copied().filter(|slot| self.uses.promoted.contains_key(slot))
    }

    pub(super) fn run(mut self) {
        let blocks: Vec<Block> = self.func.blocks.iter().map(|(b, _)| b).collect();
        let mut stores: Map<Slot, Vec<(Block, Value)>> = Map::default();
        for &block in &blocks {
            for &inst in &self.func.blocks[block].insts {
                if let InstKind::Store { value, .. } = self.func.insts[inst].kind
                    && let Some(slot) = self.promoted(&self.func.insts[inst].kind)
                {
                    self.ends.insert((block, slot), value);
                    stores.entry(slot).or_default().push((block, value));
                    *self.stored.entry(slot).or_default() += 1;
                }
            }
        }
        self.sole = stores
            .into_iter()
            .filter_map(|(slot, at)| Some((slot, *at.first().filter(|_| at.len() == 1)?)))
            .collect();
        let mut removed = Set::default();
        for &block in &blocks {
            let mut defined: Map<Slot, Value> = Map::default();
            for inst in self.func.blocks[block].insts.clone() {
                let data = &self.func.insts[inst];
                let Some(slot) = self.promoted(&data.kind) else { continue };
                removed.insert(inst);
                let step = match data.kind {
                    InstKind::Store { value, .. } => Step::Define(value),
                    InstKind::Load { .. } => Step::Read(data.result.expect("a load defines its value")),
                    _ => Step::Address,
                };
                match step {
                    Step::Define(value) => {
                        defined.insert(slot, value);
                    }
                    Step::Read(load) => {
                        let reaching = match defined.get(&slot) {
                            Some(&value) => value,
                            None => self.start(block, slot),
                        };
                        self.replaced.insert(load, reaching);
                    }
                    Step::Address => {}
                }
            }
        }
        while let Some((block, slot, param)) = self.pending.pop() {
            for pred in self.preds[block].clone() {
                let passed = self.ends.get(&(pred, slot)).copied().unwrap_or_else(|| self.start(pred, slot));
                self.incoming.entry(param).or_default().push((pred, passed));
            }
        }
        let heaviest = self.made.iter().max_by_key(|&(slot, made)| (*made, *slot));
        vun_trace::tracing::trace!(
            insts = self.func.insts.len(),
            steps = self.steps,
            parameters = self.made.values().sum::<usize>(),
            heaviest = heaviest.map_or(0, |(_, made)| *made),
            stores = heaviest.map_or(0, |(slot, _)| self.stored.get(slot).copied().unwrap_or(0)),
            at = heaviest
                .map_or(String::new(), |(slot, _)| self.func.slots[*slot].origin.located().to_string()),
            "slots reached through blocks"
        );
        self.drop_trivial();
        self.rewrite(&removed);
    }

    /// The definition of `slot` reaching the start of `block`: that reaching the end of its only
    /// predecessor, and a parameter where several meet.
    fn start(&mut self, block: Block, slot: Slot) -> Value {
        let mut path = Vec::new();
        let mut at = block;
        let value = loop {
            if let Some(&value) = self.starts.get(&(at, slot)) {
                break value;
            }
            if let Some(&(def, value)) = self.sole.get(&slot)
                && def != at
                && self.dominators.dominates(def, at)
            {
                break value;
            }
            path.push(at);
            self.steps += 1;
            match self.preds[at][..] {
                [] => break self.unset(slot),
                [pred] if path.len() <= self.preds.len() => match self.ends.get(&(pred, slot)) {
                    Some(&value) => break value,
                    None => at = pred,
                },
                _ => break self.parameter(at, slot),
            }
        };
        for visited in path {
            self.starts.insert((visited, slot), value);
        }
        value
    }

    /// A parameter of `block` for `slot`, whose predecessors pass it their definitions.
    fn parameter(&mut self, block: Block, slot: Slot) -> Value {
        *self.made.entry(slot).or_default() += 1;
        let ty = self.uses.promoted[&slot];
        let param = self.func.values.push(ValueData { ty, def: Def::Param(block, 0) });
        self.params.entry(block).or_default().push((slot, param));
        self.pending.push((block, slot, param));
        param
    }

    fn unset(&mut self, slot: Slot) -> Value {
        if let Some(&(_, value)) = self.unset.get(&slot) {
            return value;
        }
        let ty = self.uses.promoted[&slot];
        let origin = self.func.slots[slot].origin;
        let inst =
            self.func.insts.push(InstData { kind: InstKind::Const { ty, bits: 0 }, result: None, origin });
        let value = self.func.values.push(ValueData { ty, def: Def::Inst(inst) });
        self.func.insts[inst].result = Some(value);
        self.unset.insert(slot, (inst, value));
        value
    }

    /// Replaces each parameter all of whose predecessors pass one value, or itself, by that
    /// value, until none is left: a replacement asks again for the parameters it was passed to.
    fn drop_trivial(&mut self) {
        let mut params: Vec<(Slot, Value)> = self.params.values().flatten().copied().collect();
        params.sort_unstable_by_key(|&(_, param)| std::cmp::Reverse(param));
        let slots: Map<Value, Slot> = params.iter().map(|&(slot, param)| (param, slot)).collect();
        let mut receivers: Map<Value, Vec<Value>> = Map::default();
        for &(_, param) in &params {
            for &(_, passed) in &self.incoming[&param] {
                receivers.entry(passed).or_default().push(param);
            }
        }
        let mut asked: Vec<Value> = params.iter().map(|&(_, param)| param).collect();
        while let Some(param) = asked.pop() {
            if self.replaced.contains(param) {
                continue;
            }
            let passed: Set<Value> = self.incoming[&param]
                .iter()
                .map(|&(_, v)| self.replaced.resolve(v))
                .filter(|&v| v != param)
                .collect();
            let one = match passed.len() {
                0 => self.unset(slots[&param]),
                1 => *passed.iter().next().expect("one"),
                _ => continue,
            };
            self.replaced.insert(param, one);
            let passed_on = receivers.remove(&param).unwrap_or_default();
            asked.extend(passed_on.iter().copied());
            receivers.entry(one).or_default().extend(passed_on);
        }
    }

    /// Adds the parameters kept to their blocks and their values to the jumps to them, removes
    /// the promoted instructions, and replaces what was replaced everywhere.
    fn rewrite(mut self, removed: &Set<Inst>) {
        let replaced = std::mem::take(&mut self.replaced);
        let mut blocks: Vec<(Block, Vec<(Slot, Value)>)> =
            std::mem::take(&mut self.params).into_iter().collect();
        blocks.sort_unstable_by_key(|&(block, _)| block);
        for (block, params) in blocks {
            let kept: Vec<Value> =
                params.into_iter().map(|(_, p)| p).filter(|p| !replaced.contains(*p)).collect();
            for &pred in &self.preds[block] {
                let passed: Vec<Value> = kept
                    .iter()
                    .map(|p| {
                        let (_, from) =
                            self.incoming[p].iter().find(|(b, _)| *b == pred).expect("each passes");
                        replaced.resolve(*from)
                    })
                    .collect();
                for target in self.func.blocks[pred].term.targets_mut().filter(|t| t.block == block) {
                    target.args.extend(&passed);
                }
            }
            self.func.blocks[block].params.extend(kept);
        }
        let entry = self.func.entry().expect("a function with blocks");
        let mut unset: Vec<Inst> = self.unset.values().map(|&(inst, _)| inst).collect();
        unset.sort_unstable();
        self.func.blocks[entry].insts.splice(0..0, unset);
        let resolve = |v| replaced.resolve(v);
        for (_, data) in self.func.blocks.iter_mut() {
            data.insts.retain(|inst| !removed.contains(inst));
            for &inst in &data.insts {
                self.func.insts[inst].kind.map_values(resolve);
            }
            data.term.map_values(resolve);
        }
        self.func.compact();
    }
}
