//! Inlining small functions (spec 13, section 3): a call of an internal function of Vun's
//! convention of few instructions, constants aside, is replaced by a copy of its blocks. The block calling ends in
//! a jump to the copy of the entry, passing the call's arguments, and what followed the call
//! moves to a block of its own that each return jumps to, passing the value returned, which is
//! the call's value. What a call costs a function this small, its arguments, frame and the
//! values stored around it, is most of what it does, and its body then simplifies with what the
//! caller knows. The instructions keep their origins, so line tables still name the callee's
//! lines.

#[cfg(test)]
mod tests;

use vun_hash::Map;
use vun_index::Idx;

use crate::entity::{Block, FuncId, Slot, Value};
use crate::func::{BlockData, Conv, Def, Function, Linkage, ValueData};
use crate::inst::{Callee, InstKind, Target, Terminator};
use crate::unit::Unit;

/// The most instructions a function inlined may have, constants aside.
const SMALL: usize = 96;

impl Unit {
    /// Inlines every call of a small function (see the module), callees first so a function
    /// inlined has its own small callees inlined already, returning how many were.
    pub fn inline(&mut self) -> usize {
        self.callees_first().into_iter().map(|f| self.inline_into(f)).sum()
    }

    /// Inlines the calls `f` makes of small functions, as they are now, returning how many were.
    pub fn inline_into(&mut self, f: FuncId) -> usize {
        let mut small: Map<FuncId, Function> = Map::default();
        for g in self.functions[f].callees() {
            if g != f && !small.contains_key(&g) && self.functions[g].small() {
                small.insert(g, self.functions[g].clone());
            }
        }
        if small.is_empty() {
            return 0;
        }
        self.functions[f].inline_calls(&small)
    }
}

impl Function {
    /// Whether calls of this function are replaced by its body.
    fn small(&self) -> bool {
        self.linkage == Linkage::Internal && self.signature.conv == Conv::Vun && self.cost() <= SMALL
    }

    /// The instructions of the body but its constants, which code generation computes where
    /// they are used.
    fn cost(&self) -> usize {
        let insts = self.blocks.iter().flat_map(|(_, b)| &b.insts);
        insts.filter(|&&i| !matches!(self.insts[i].kind, InstKind::Const { .. })).count()
    }

    /// Replaces each call of a function of `small` by its body, in the blocks this has now and
    /// those holding what followed a call; not in the bodies inlined, so a function calling
    /// itself is inlined once.
    fn inline_calls(&mut self, small: &Map<FuncId, Function>) -> usize {
        let mut count = 0;
        let mut pending: Vec<Block> = self.blocks.iter().map(|(b, _)| b).collect();
        while let Some(block) = pending.pop() {
            let mut insts = std::mem::take(&mut self.blocks[block].insts).into_iter();
            let mut kept = Vec::with_capacity(insts.len());
            while let Some(inst) = insts.next() {
                let callee = match &self.insts[inst].kind {
                    InstKind::Call { callee: Callee::Func(f), args } => {
                        small.get(f).map(|s| (s, args.clone()))
                    }
                    _ => None,
                };
                let Some((body, args)) = callee else {
                    kept.push(inst);
                    continue;
                };
                let data = &self.blocks[block];
                let rest = BlockData {
                    params: Vec::new(),
                    insts: insts.collect(),
                    term: data.term.clone(),
                    origin: data.origin,
                };
                let after = self.blocks.push(rest);
                if let Some(result) = self.insts[inst].result {
                    self.blocks[after].params.push(result);
                    self.values[result].def = Def::Param(after, 0);
                }
                let entry = self.splice(body, after);
                self.blocks[block].term = Terminator::Jump(Target { block: entry, args });
                pending.push(after);
                count += 1;
                break;
            }
            self.blocks[block].insts = kept;
        }
        if count > 0 {
            self.compact();
        }
        count
    }

    /// Copies the blocks of `body` into this function, its slots new ones, each return a jump to
    /// `after` passing the value returned; gives the copy of its entry.
    fn splice(&mut self, body: &Function, after: Block) -> Block {
        let blocks: Vec<Block> = body
            .blocks
            .iter()
            .map(|(_, data)| {
                let copy = BlockData {
                    params: Vec::new(),
                    insts: Vec::new(),
                    term: Terminator::Unreachable,
                    origin: data.origin,
                };
                self.blocks.push(copy)
            })
            .collect();
        let slots: Vec<Slot> = body.slots.iter().map(|(_, slot)| self.slots.push(*slot)).collect();
        let mut values: Vec<Option<Value>> = vec![None; body.values.len()];
        // Every value first, so each operand has its copy whatever order the blocks are in.
        for (old, data) in body.blocks.iter() {
            let new = blocks[old.index()];
            for (i, &param) in data.params.iter().enumerate() {
                let value = self.values.push(ValueData { ty: body.ty(param), def: Def::Param(new, i) });
                self.blocks[new].params.push(value);
                values[param.index()] = Some(value);
            }
            for &inst in &data.insts {
                let data = &body.insts[inst];
                let (copy, value) =
                    self.define(data.kind.clone(), data.result.map(|r| body.ty(r)), data.origin);
                if let Some(result) = data.result {
                    values[result.index()] = value;
                }
                self.blocks[new].insts.push(copy);
            }
        }
        let copied = |v: Value| values[v.index()].expect("every value of the body is copied");
        for (old, data) in body.blocks.iter() {
            let new = blocks[old.index()];
            for &inst in &self.blocks[new].insts {
                let kind = &mut self.insts[inst].kind;
                kind.map_values(copied);
                if let InstKind::SlotAddr(slot) = kind {
                    *slot = slots[slot.index()];
                }
            }
            let term = match &data.term {
                Terminator::Return(value) => Terminator::Jump(Target {
                    block: after,
                    args: value.iter().map(|&v| copied(v)).collect(),
                }),
                term => {
                    let mut term = term.clone();
                    term.map_values(copied);
                    for target in term.targets_mut() {
                        target.block = blocks[target.block.index()];
                    }
                    term
                }
            };
            self.blocks[new].term = term;
        }
        blocks[body.entry().expect("a function inlined has a body").index()]
    }
}
