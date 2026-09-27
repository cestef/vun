//! Callee-saved registers: those the plan holds values in are saved by the prologue, below the
//! frame pointer, and restored by each return.

use vun_dwarf::Step;

use super::Cx;
use crate::Arch;

impl<A: Arch> Cx<'_, A> {
    /// Saves the callee-saved registers the plan uses.
    pub(super) fn save(&mut self) {
        // The caller's frame starts two words above the frame pointer, past the frame pointer
        // and the return address saved.
        let above = 2 * i64::try_from(A::WORD).expect("a word fits");
        let used = self.plan().used.intersection(self.machine.preserved);
        for p in used.iter() {
            let r = self.reg_of(p);
            let save = self.frame.reserve(A::WORD, A::WORD);
            A::store(&mut self.asm, save, r);
            self.unwound(Step::Saved { reg: A::dwarf(r), offset: i64::from(save) - above });
            self.saves.push((r, save));
        }
    }

    /// Restores the callee-saved registers taken, before returning.
    pub fn restore(&mut self) {
        for &(r, save) in &self.saves {
            A::load(&mut self.asm, r, save);
        }
    }
}
