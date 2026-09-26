//! Inlining small functions (spec 13, section 3): a call of an internal function of Vun's
//! convention whose entry block returns, after few instructions, is replaced by
//! those instructions, its parameters the call's arguments and its result the value returned.
//! What a call costs a function this small, its arguments, frame and the values stored around
//! it, is most of what it does. The instructions keep their origins, so line tables still name
//! the callee's lines.

#[cfg(test)]
mod tests;

use vun_index::Idx;

use crate::entity::{FuncId, Inst, Slot, Value};
use crate::func::{Conv, Def, Function, InstData, Linkage, ValueData};
use crate::inst::{Callee, InstKind, Terminator};
use crate::replaced::Replaced;
use crate::unit::Unit;

/// The most instructions a function inlined may have.
const SMALL: usize = 8;

/// A function a call of which is replaced by its body.
struct Inlined {
    body: Function,
}

impl Unit {
    /// Inlines every call of a small function (see the module), returning how many were.
    pub fn inline(&mut self) -> usize {
        let small: Vec<Option<Inlined>> =
            self.functions.iter().map(|(_, f)| f.small().then(|| Inlined { body: f.clone() })).collect();
        let mut count = 0;
        for (id, function) in self.functions.iter_mut() {
            count += function.inline_calls(id, &small);
        }
        count
    }
}

impl Function {
    /// Whether calls of this function are replaced by its body.
    fn small(&self) -> bool {
        let Some(entry) = self.entry() else { return false };
        // An entry that returns is the whole function: any other block is unreached.
        self.linkage == Linkage::Internal
            && self.signature.conv == Conv::Vun
            && matches!(self.blocks[entry].term, Terminator::Return(_))
            && self.blocks[entry].insts.len() <= SMALL
    }

    /// Replaces each call of a function of `small` by its body; `id` is this function, never
    /// inlined into itself.
    fn inline_calls(&mut self, id: FuncId, small: &[Option<Inlined>]) -> usize {
        let mut replaced = Replaced::default();
        let mut count = 0;
        let blocks: Vec<_> = self.blocks.iter().map(|(b, _)| b).collect();
        for block in blocks {
            let mut insts = Vec::with_capacity(self.blocks[block].insts.len());
            for inst in std::mem::take(&mut self.blocks[block].insts) {
                let callee = match &self.insts[inst].kind {
                    InstKind::Call { callee: Callee::Func(f), args } if *f != id => {
                        small.get(f.index()).and_then(Option::as_ref).map(|s| (s, args.clone()))
                    }
                    _ => None,
                };
                let Some((inlined, args)) = callee else {
                    insts.push(inst);
                    continue;
                };
                let args: Vec<Value> = args.iter().map(|&a| replaced.resolve(a)).collect();
                let returned = self.splice(&inlined.body, &args, &mut insts);
                if let (Some(result), Some(value)) = (self.insts[inst].result, returned) {
                    replaced.insert(result, value);
                }
                count += 1;
            }
            self.blocks[block].insts = insts;
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

    /// Appends the instructions of `body`'s one block to `insts`, its parameters `args`, its
    /// slots new ones; gives the value it returns.
    fn splice(&mut self, body: &Function, args: &[Value], insts: &mut Vec<Inst>) -> Option<Value> {
        let entry = body.entry().expect("a small function has its block");
        let mut values: Vec<Option<Value>> = vec![None; body.values.len()];
        for (&param, &arg) in body.blocks[entry].params.iter().zip(args) {
            values[param.index()] = Some(arg);
        }
        let slots: Vec<Slot> = body.slots.iter().map(|(_, slot)| self.slots.push(*slot)).collect();
        for &old in &body.blocks[entry].insts {
            let data = &body.insts[old];
            let mut kind = data.kind.clone();
            kind.map_values(|v| values[v.index()].expect("defined before it is read"));
            if let InstKind::SlotAddr(slot) = &mut kind {
                *slot = slots[slot.index()];
            }
            let new = self.insts.push(InstData { kind, result: None, origin: data.origin });
            if let Some(result) = data.result {
                let value = self.values.push(ValueData { ty: body.ty(result), def: Def::Inst(new) });
                self.insts[new].result = Some(value);
                values[result.index()] = Some(value);
            }
            insts.push(new);
        }
        match body.blocks[entry].term {
            Terminator::Return(Some(v)) => values[v.index()],
            _ => None,
        }
    }
}
