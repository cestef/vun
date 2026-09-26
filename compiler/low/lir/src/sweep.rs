//! Sweeping what a function no longer needs after its other passes: the blocks control never
//! reaches are emptied, then every instruction that only defines a value nothing reads goes,
//! with what only it read. Lowering guarantees each operation's domain (`facts`), so dropping
//! one never drops a trap.

use crate::entity::Inst;
use crate::func::{Def, Function};
use crate::inst::Terminator;

impl Function {
    /// Empties the unreached blocks and removes the instructions that do nothing but define an
    /// unread value (`InstKind::acts`), returning how many instructions went.
    pub fn sweep(&mut self) -> usize {
        let reached = self.reached();
        let mut count = 0;
        for (block, data) in self.blocks.iter_mut() {
            if !reached[block] && (!data.insts.is_empty() || data.term != Terminator::Unreachable) {
                count += data.insts.len();
                data.insts.clear();
                data.term = Terminator::Unreachable;
            }
        }
        let mut uses = self.uses();
        let unread = |f: &Self, inst: Inst, uses: &vun_index::IndexVec<_, u32>| {
            let data = &f.insts[inst];
            !data.kind.acts() && data.result.is_none_or(|v| uses[v] == 0)
        };
        let mut pending: Vec<Inst> = self
            .blocks
            .iter()
            .flat_map(|(_, b)| b.insts.iter().copied())
            .filter(|&i| unread(self, i, &uses))
            .collect();
        let mut dropped = vun_hash::Set::default();
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
        let count = count + dropped.len();
        if count > 0 {
            self.compact();
        }
        count
    }
}
