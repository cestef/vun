//! Compacting a function after a pass removed instructions from its blocks or parameters from
//! them: what no block holds goes, with the values it defined and the slots nothing addresses,
//! and every handle is renumbered in the order of what remains.

use vun_index::{Idx, IndexVec};

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
        let mut addressed: IndexVec<Slot, bool> = IndexVec::filled(false, self.slots.len());
        for (_, data) in self.blocks.iter() {
            for &inst in &data.insts {
                if let InstKind::SlotAddr(slot) = self.insts[inst].kind {
                    addressed[slot] = true;
                }
            }
        }
        let mut slots: IndexVec<Slot, Option<Slot>> = IndexVec::filled(None, self.slots.len());
        let mut kept_slots = IndexVec::<Slot, SlotData>::new();
        for (old, slot) in self.slots.iter().filter(|(old, _)| addressed[*old]) {
            slots[old] = Some(kept_slots.push(*slot));
        }
        // The old instructions, each moved out once to where it is kept.
        let mut old: Vec<Option<InstData>> =
            std::mem::take(&mut self.insts).into_vec().into_iter().map(Some).collect();
        let mut insts = IndexVec::<Inst, InstData>::new();
        let mut values = IndexVec::<Value, ValueData>::new();
        let mut renamed: IndexVec<Value, Option<Value>> = IndexVec::filled(None, self.values.len());
        for (block, data) in self.blocks.iter_mut() {
            for (i, param) in data.params.iter_mut().enumerate() {
                let new = values.push(ValueData { ty: self.values[*param].ty, def: Def::Param(block, i) });
                renamed[*param] = Some(new);
                *param = new;
            }
            for inst in &mut data.insts {
                let mut kept = old[inst.index()].take().expect("a block holds an instruction once");
                let new = Inst::new(insts.len());
                if let Some(result) = kept.result {
                    let value = values.push(ValueData { ty: self.values[result].ty, def: Def::Inst(new) });
                    renamed[result] = Some(value);
                    kept.result = Some(value);
                }
                if let InstKind::SlotAddr(slot) = &mut kept.kind {
                    *slot = slots[*slot].expect("an addressed slot is kept");
                }
                *inst = insts.push(kept);
            }
        }
        let rename = |v: Value| {
            renamed.get(v).copied().flatten().unwrap_or_else(|| panic!("{v} is used but no longer defined"))
        };
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
