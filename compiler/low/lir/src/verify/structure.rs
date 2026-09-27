//! The shape of a function: each value defined where its definition says, and used only so, the
//! entry entered only by a call, the cases of a switch distinct, and slots aligned to powers of
//! two.

use crate::entity::{Block, Value};
use crate::func::Def;
use crate::inst::Terminator;

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
        Ok(())
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
