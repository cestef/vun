//! Callee-saved registers for loops (spec 13, section 8.3): the values live across the blocks of a
//! loop, deepest loops and most uses first, are each held in a callee-saved register of their own
//! from their definition to their last use. Calls preserve them and block ends keep them, so a
//! loop runs without storing them home; the prologue saves the registers taken, and each return
//! restores them.

use vun_lir::Value;
use vun_lir::analysis::{Liveness, Loops};

use super::super::abi::Loc;
use super::super::{Mem, Reg, Size};
use super::{FnCompiler, HOME};

impl FnCompiler<'_> {
    /// Chooses the values held in callee-saved registers, saving each register taken.
    pub(super) fn fix(&mut self, live: &Liveness, loops: &Loops) {
        let depth = |block| std::iter::successors(loops.innermost(block), |&h| loops.parent(h)).count();
        let blocks = self.order.blocks().to_vec();
        let entry: Vec<Value> = self.f.entry().map(|b| self.f.blocks[b].params.clone()).unwrap_or_default();
        let mut candidates: Vec<(usize, u32, Value)> = self
            .f
            .values
            .iter()
            .filter_map(|(v, _)| {
                let range = live.range(v)?;
                let spans = range.last > range.first || range.through;
                // A parameter passed in memory is recomputed as its address.
                let in_memory = entry
                    .iter()
                    .position(|&p| p == v)
                    .and_then(|i| self.abi.params.get(i))
                    .is_some_and(|loc| !matches!(loc, Loc::Reg(_) | Loc::Stack(_)));
                if !spans || in_memory || self.remat(v) || Size::of(self.f.ty(v)).is_none() {
                    return None;
                }
                let deepest = (range.first..=range.last).map(|p| depth(blocks[p as usize])).max()?;
                (deepest > 0).then_some((deepest, range.uses, v))
            })
            .collect();
        candidates.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)).then(a.2.cmp(&b.2)));
        for (&(_, _, v), r) in candidates.iter().zip(Reg::CALLEE_SAVED) {
            let save = self.reserve(HOME, HOME);
            self.asm.store(Size::B64, Mem::at(Reg::Rbp, save), r);
            self.saves.push((r, save));
            self.fixed[v] = Some(r);
            self.holds[r.number() as usize] = Some(v);
            self.reg[v] = Some(r);
        }
    }

    /// Whether `r` holds a value of its own for the whole function.
    pub(super) fn is_fixed(&self, r: Reg) -> bool {
        self.holds[r.number() as usize].is_some_and(|v| self.fixed[v] == Some(r))
    }

    /// Restores the callee-saved registers taken, before returning.
    pub(super) fn restore(&mut self) {
        for &(r, save) in &self.saves.clone() {
            self.asm.load(Size::B64, r, Mem::at(Reg::Rbp, save));
        }
    }
}
