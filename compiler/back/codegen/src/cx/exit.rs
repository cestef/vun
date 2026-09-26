//! Leaving a block. Only the state at the current point is tracked, so a target entered from
//! elsewhere finds every value still used in its home (or its own register), and each jump fills
//! its target's parameters by the sequenced moves of `Moves`. A block emitted next that only this
//! one reaches starts with the registers as they are left, and a jump into it stores nothing.

use vun_lir::analysis::{Location, Moves};
use vun_lir::{Block, Cond, InstKind, Target, Terminator, Value};

use super::Cx;
use crate::{Arch, Isel};

/// A comparison made by the branch ending its block, the flags never materialized.
#[derive(Clone, Copy, Debug)]
pub struct Fused {
    /// The comparison's result, which nothing else reads.
    pub value: Value,
    pub cond: Cond,
    pub a: Value,
    pub b: Value,
}

/// How a branch is laid out: the target it jumps to where its condition holds, or where it does
/// not when inverted, and the one it reaches otherwise, emitted next when it can be.
pub struct Layout<'t> {
    pub taken: &'t Target,
    pub fall: &'t Target,
    /// Whether it jumps where the condition does not hold: `then` is emitted next.
    pub inverted: bool,
}

impl<'t> Layout<'t> {
    /// Falls into `then` when it is emitted next and `otherwise` is not, into `otherwise` else.
    #[must_use]
    pub fn of(then: &'t Target, otherwise: &'t Target, next: Option<Block>) -> Self {
        let inverted = next == Some(then.block) && next != Some(otherwise.block);
        if inverted {
            Self { taken: otherwise, fall: then, inverted }
        } else {
            Self { taken: then, fall: otherwise, inverted }
        }
    }
}

impl<A: Isel> Cx<'_, A> {
    /// The comparison `block` branches on, when it is made in `block`, the branch is its one
    /// use and the architecture branches on its flags.
    #[must_use]
    pub fn fused(&self, block: Block) -> Option<Fused> {
        let Terminator::Branch { cond: value, .. } = self.f.blocks[block].term else { return None };
        let vun_lir::func::Def::Inst(inst) = self.f.values[value].def else { return None };
        let InstKind::Compare { cond, a, b } = self.f.insts[inst].kind else { return None };
        let here = self.f.blocks[block].insts.contains(&inst);
        (here && self.values[value].left == 1 && A::fuses(cond)).then_some(Fused { value, cond, a, b })
    }
}

impl<A: Arch> Cx<'_, A> {
    /// Leaves `block`, its terminator's operands read: counts their uses, stores home what a
    /// target entered from elsewhere reads there, and gives the block emitted next.
    pub fn leave(&mut self, block: Block) -> Option<Block> {
        let term = &self.f.blocks[block].term;
        for v in term.operands() {
            self.used(v);
        }
        let next = self.order.next(block);
        let falls = |t: &Target| next == Some(t.block) && self.cfg.preds[t.block].len() == 1;
        let (keep, only) = match term {
            Terminator::Jump(t) => (falls(t), falls(t)),
            Terminator::Branch { then, otherwise, .. } => {
                (falls(Layout::of(then, otherwise, next).fall), false)
            }
            _ => (false, false),
        };
        if !only {
            self.spill_live();
        }
        self.kept = if keep { next } else { None };
        next
    }

    /// After a block's last jump: the block emitted next keeps the registers of the values it
    /// still uses, if it may.
    pub fn left(&mut self) {
        if self.kept.is_none() {
            self.clear();
        } else {
            self.free_dead();
        }
        self.unlock();
    }

    /// Fills `target`'s parameters, then jumps unless it is `next`.
    pub fn jump(&mut self, target: &Target, next: Option<Block>) {
        for m in Moves::of(self.f, target).0 {
            // A parameter in its own register is written there directly.
            if let (Location::Value(v), Location::Value(p)) = (m.from, m.to)
                && let Some(r) = self.values[p].fixed
            {
                self.reg_to(v, r);
                continue;
            }
            let carrier = match m.to {
                Location::Temp(_) => A::TEMP,
                Location::Value(_) => A::CARRIER,
            };
            let from = match m.from {
                Location::Value(v) => {
                    self.reg_to(v, carrier);
                    carrier
                }
                Location::Temp(_) => A::TEMP,
            };
            if let Location::Value(p) = m.to {
                if let Some(r) = self.values[p].fixed {
                    A::copy(&mut self.asm, r, from);
                } else {
                    let home = self.home(p);
                    A::store(&mut self.asm, home, from);
                }
            }
        }
        if next != Some(target.block) {
            A::jump(&mut self.asm, self.labels[target.block]);
        }
    }
}
