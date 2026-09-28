//! The shape of a function: each value defined where its definition says, and used only so, the
//! entry entered only by a call, the cases of a switch distinct, slots aligned to powers of two,
//! and a naked function nothing but assembly without bindings.

use crate::entity::{Block, Value};
use crate::func::{Conv, Def};
use crate::inst::{InstKind, Terminator};

use super::{Checker, Entity, Malformed, Reason};

impl Checker<'_> {
    pub(super) fn structure(&self, entry: Block) -> Result<(), Malformed> {
        let id = self.id;
        let defined = self.func.insts.iter().filter_map(|(inst, d)| Some((d.result?, Def::Inst(inst))));
        let params = self.func.blocks.iter().flat_map(|(block, data)| {
            data.params.iter().enumerate().map(move |(i, &value)| (value, Def::Param(block, i)))
        });
        for (value, def) in defined.chain(params) {
            if self.func.values.get(value).is_none_or(|data| data.def != def) {
                return Self::fail(Entity::Func(id), Reason::Definition(value));
            }
        }
        for (slot, data) in self.func.slots.iter() {
            if !data.align.is_power_of_two() {
                return Self::fail(Entity::Slot(id, slot), Reason::Alignment(data.align));
            }
        }
        for (block, data) in self.func.blocks.iter() {
            let at = Entity::Block(id, block);
            if data.term.targets().any(|t| t.block == entry) {
                return Self::fail(at, Reason::EntryTargeted);
            }
            if let Terminator::Switch { cases, .. } = &data.term
                && let Some(i) = (1..cases.len()).find(|&i| cases[..i].iter().any(|c| c.0 == cases[i].0))
            {
                return Self::fail(at, Reason::DuplicateCase(cases[i].0));
            }
        }
        if self.func.signature.conv == Conv::Naked {
            self.naked()?;
        }
        Ok(())
    }

    /// A naked function has no frame to hold a slot or return through, and no registers the
    /// backend may give a binding: its code is its blocks' instructions as written.
    fn naked(&self) -> Result<(), Malformed> {
        let id = self.id;
        if let Some((slot, _)) = self.func.slots.iter().next() {
            return Self::fail(Entity::Slot(id, slot), Reason::Naked);
        }
        for (inst, data) in self.func.insts.iter() {
            if !matches!(&data.kind, InstKind::Asm(asm, None) if asm.bindings().next().is_none()) {
                return Self::fail(Entity::Inst(id, inst), Reason::Naked);
            }
        }
        match self.func.blocks.iter().find(|(_, data)| matches!(data.term, Terminator::Return(_))) {
            Some((block, _)) => Self::fail(Entity::Block(id, block), Reason::Naked),
            None => Ok(()),
        }
    }

    /// Whether `def` gives `value`: a value no longer defined may stay in the arena until the
    /// function is compacted, but not be used.
    pub(super) fn defines(&self, def: Def, value: Value) -> bool {
        match def {
            Def::Param(block, i) => self.func.blocks.get(block).and_then(|b| b.params.get(i)) == Some(&value),
            Def::Inst(inst) => self.func.insts.get(inst).is_some_and(|d| d.result == Some(value)),
        }
    }
}
