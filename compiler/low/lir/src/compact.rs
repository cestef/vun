//! Compacting a function after a pass removed instructions from its blocks or parameters from
//! them: what no block holds goes, with the values it defined and the slots nothing addresses,
//! and every handle is renumbered in the order of what remains.

use std::collections::{HashMap, HashSet};

use vun_index::IndexVec;

use crate::entity::{Inst, Slot, Value};
use crate::func::{Def, Function, InstData, SlotData, ValueData};
use crate::inst::InstKind;

impl Function {
    /// Keeps the instructions blocks hold, the values their parameters and instructions define,
    /// and the slots an instruction addresses, each in its old order.
    ///
    /// # Panics
    /// If a kept operand is a value no longer defined: a pass removed a definition it left uses of.
    pub fn compact(&mut self) {
        let mut slots: HashMap<Slot, Slot> = HashMap::new();
        let mut kept_slots = IndexVec::<Slot, SlotData>::new();
        let mut insts = IndexVec::<Inst, InstData>::new();
        let mut values = IndexVec::<Value, ValueData>::new();
        let mut renamed: HashMap<Value, Value> = HashMap::new();
        let addressed: HashSet<Slot> = self
            .blocks
            .iter()
            .flat_map(|(_, b)| &b.insts)
            .filter_map(|&inst| match self.insts[inst].kind {
                InstKind::SlotAddr(slot) => Some(slot),
                _ => None,
            })
            .collect();
        for (old, slot) in self.slots.iter().filter(|(old, _)| addressed.contains(old)) {
            slots.insert(old, kept_slots.push(*slot));
        }
        for (block, data) in self.blocks.iter_mut() {
            for (i, param) in data.params.iter_mut().enumerate() {
                let new = values.push(ValueData { ty: self.values[*param].ty, def: Def::Param(block, i) });
                renamed.insert(*param, new);
                *param = new;
            }
            for inst in &mut data.insts {
                let mut kept = self.insts[*inst].clone();
                let new = insts.push(kept.clone());
                if let Some(result) = kept.result {
                    let value = values.push(ValueData { ty: self.values[result].ty, def: Def::Inst(new) });
                    renamed.insert(result, value);
                    kept.result = Some(value);
                }
                if let InstKind::SlotAddr(slot) = &mut kept.kind {
                    *slot = slots[slot];
                }
                insts[new] = kept;
                *inst = new;
            }
        }
        let rename =
            |v: Value| *renamed.get(&v).unwrap_or_else(|| panic!("{v} is used but no longer defined"));
        for (_, inst) in insts.iter_mut() {
            inst.kind.map_values(rename);
        }
        for (_, data) in self.blocks.iter_mut() {
            data.term.map_values(rename);
        }
        self.slots = kept_slots;
        self.insts = insts;
        self.values = values;
    }
}
