//! The construction: each load becomes the definition reaching it, a block where several reach
//! gets a parameter, filled by each predecessor with the definition at its end, and parameters
//! every predecessor fills with one value (or with themselves) are that value.

use std::collections::{HashMap, HashSet};

use vun_index::IndexVec;

use super::Uses;
use crate::analysis::Cfg;
use crate::entity::{Block, Inst, Slot, Value};
use crate::func::{Def, Function, InstData, ValueData};
use crate::inst::InstKind;

/// Loads and trivial parameters, by what stands for them.
#[derive(Default)]
struct Replaced(HashMap<Value, Value>);

impl Replaced {
    /// What stands for `value` in the end.
    fn resolve(&self, mut value: Value) -> Value {
        while let Some(&next) = self.0.get(&value) {
            value = next;
        }
        value
    }
}

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
    ends: HashMap<(Block, Slot), Value>,
    /// The definition of each slot reaching the start of each block, once asked for.
    starts: HashMap<(Block, Slot), Value>,
    /// The parameters added to each block, in order, with their slots.
    params: HashMap<Block, Vec<(Slot, Value)>>,
    /// Parameters whose predecessors' definitions are not asked for yet.
    pending: Vec<(Block, Slot, Value)>,
    /// What each predecessor passes each parameter.
    incoming: HashMap<Value, Vec<(Block, Value)>>,
    replaced: Replaced,
    /// The value of a slot read before any store, a constant at the entry, once asked for.
    unset: HashMap<Slot, (Inst, Value)>,
}

impl<'f> Promotion<'f> {
    pub(super) fn new(func: &'f mut Function, uses: Uses) -> Self {
        let preds = Cfg::of(func).preds;
        Self {
            func,
            uses,
            preds,
            ends: HashMap::new(),
            starts: HashMap::new(),
            params: HashMap::new(),
            pending: Vec::new(),
            incoming: HashMap::new(),
            replaced: Replaced::default(),
            unset: HashMap::new(),
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
        for &block in &blocks {
            for &inst in &self.func.blocks[block].insts {
                if let InstKind::Store { value, .. } = self.func.insts[inst].kind
                    && let Some(slot) = self.promoted(&self.func.insts[inst].kind)
                {
                    self.ends.insert((block, slot), value);
                }
            }
        }
        let mut removed = HashSet::new();
        for &block in &blocks {
            let mut defined: HashMap<Slot, Value> = HashMap::new();
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
                        self.replaced.0.insert(load, reaching);
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
        self.drop_trivial();
        self.rewrite(&removed);
    }

    /// The definition of `slot` reaching the start of `block`.
    fn start(&mut self, block: Block, slot: Slot) -> Value {
        if let Some(&value) = self.starts.get(&(block, slot)) {
            return value;
        }
        let value = if self.preds[block].is_empty() {
            self.unset(slot)
        } else {
            let ty = self.uses.promoted[&slot];
            let param = self.func.values.push(ValueData { ty, def: Def::Param(block, 0) });
            self.params.entry(block).or_default().push((slot, param));
            self.pending.push((block, slot, param));
            param
        };
        self.starts.insert((block, slot), value);
        value
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
    /// value, until none is left.
    fn drop_trivial(&mut self) {
        let mut params: Vec<(Slot, Value)> = self.params.values().flatten().copied().collect();
        params.sort_unstable_by_key(|&(_, param)| param);
        let mut changed = true;
        while changed {
            changed = false;
            for &(slot, param) in &params {
                if self.replaced.0.contains_key(&param) {
                    continue;
                }
                let passed: HashSet<Value> = self.incoming[&param]
                    .iter()
                    .map(|&(_, v)| self.replaced.resolve(v))
                    .filter(|&v| v != param)
                    .collect();
                let one = match passed.len() {
                    0 => self.unset(slot),
                    1 => *passed.iter().next().expect("one"),
                    _ => continue,
                };
                self.replaced.0.insert(param, one);
                changed = true;
            }
        }
    }

    /// Adds the parameters kept to their blocks and their values to the jumps to them, removes
    /// the promoted instructions, and replaces what was replaced everywhere.
    fn rewrite(mut self, removed: &HashSet<Inst>) {
        let replaced = std::mem::take(&mut self.replaced);
        let mut blocks: Vec<(Block, Vec<(Slot, Value)>)> =
            std::mem::take(&mut self.params).into_iter().collect();
        blocks.sort_unstable_by_key(|&(block, _)| block);
        for (block, params) in blocks {
            let kept: Vec<Value> =
                params.into_iter().map(|(_, p)| p).filter(|p| !replaced.0.contains_key(p)).collect();
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
