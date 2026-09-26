//! Callee-saved registers for loops (spec 13, section 8.3): the values live across the blocks of a
//! loop, and the parameters of a loop's head, deepest loops and most uses first, are each held in a callee-saved register of their own
//! from their definition to their last use. Calls preserve them and block ends keep them, so a
//! loop runs without storing them home; the prologue saves the registers taken, and each return
//! restores them.

use vun_lir::Value;
use vun_lir::analysis::{Liveness, Loops};
use vun_lir::func::Def;

use vun_dwarf::Step;

use super::Cx;
use crate::Arch;

impl<A: Arch> Cx<'_, A> {
    /// Chooses the values held in callee-saved registers, saving each register taken.
    pub(super) fn fix(&mut self, live: &Liveness, loops: &Loops) {
        let depth = |block| std::iter::successors(loops.innermost(block), |&h| loops.parent(h)).count();
        let blocks = self.order.blocks();
        let mut candidates: Vec<(usize, u32, Value)> = self
            .f
            .values
            .iter()
            .filter_map(|(v, _)| {
                let range = live.range(v)?;
                // A loop head's parameter is carried around the loop, even one ending there.
                let carried = matches!(self.f.values[v].def, Def::Param(b, _) if loops.is_header(b));
                let spans = range.last > range.first || range.through || carried;
                if !spans
                    || self.values[v].recompute.is_some()
                    || self.f.ty(v).bits(self.unit.pointer_bits) > 64
                {
                    return None;
                }
                let deepest = (range.first..=range.last).map(|p| depth(blocks[p as usize])).max()?;
                (deepest > 0).then_some((deepest, range.uses, v))
            })
            .collect();
        candidates.sort_unstable_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)).then(a.2.cmp(&b.2)));
        // The caller's frame starts two words above the frame pointer, past the frame pointer
        // and the return address saved.
        let above = 2 * i64::try_from(A::WORD).expect("a word fits");
        for (&(_, _, v), &r) in candidates.iter().zip(A::CALLEE_SAVED) {
            let save = self.frame.reserve(A::WORD, A::WORD);
            A::store(&mut self.asm, save, r);
            self.unwound(Step::Saved { reg: A::dwarf(r), offset: i64::from(save) - above });
            self.saves.push((r, save));
            self.values[v].fixed = Some(r);
            self.values[v].reg = Some(r);
            self.regs.set(r, Some(v));
        }
    }

    /// Whether `r` holds a value of its own for the whole function.
    pub(super) fn is_fixed(&self, r: A::Reg) -> bool {
        self.regs.holds(r).is_some_and(|v| self.values[v].fixed == Some(r))
    }

    /// Restores the callee-saved registers taken, before returning.
    pub fn restore(&mut self) {
        for &(r, save) in &self.saves {
            A::load(&mut self.asm, r, save);
        }
    }
}
