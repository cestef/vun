//! Hoisting constants out of loops: a constant defined inside a loop is defined once before it, at
//! the end of the block immediately dominating the header of the outermost loop around it, so a
//! backend keeping it in a register keeps it across the loop rather than making it again each
//! round. So are the addresses of stack slots and symbols, but once the other passes are done:
//! `promote` reads a slot's address where it is used. Irreducible loops, entered at more than one block,
//! are left as they are.

#[cfg(test)]
mod tests;

use std::iter;

use vun_lir::analysis::{Cfg, Dominators, Loops};
use vun_lir::entity::Inst;
use vun_lir::func::Function;
use vun_lir::inst::InstKind;

pub trait Hoist {
    /// Moves every constant out of the loops around it; returns how many moved.
    fn hoist(&mut self) -> usize;

    /// Moves every address of a slot or a symbol out of the loops around it; returns how many moved.
    fn hoist_addresses(&mut self) -> usize;

    /// Moves every instruction `movable` names out of the loops around it; returns how many moved.
    fn hoist_where(&mut self, movable: fn(&InstKind) -> bool) -> usize;
}

impl Hoist for Function {
    fn hoist(&mut self) -> usize {
        self.hoist_where(|kind| matches!(kind, InstKind::Const { .. }))
    }

    fn hoist_addresses(&mut self) -> usize {
        self.hoist_where(|kind| matches!(kind, InstKind::SlotAddr(_) | InstKind::Addr(_)))
    }

    fn hoist_where(&mut self, movable: fn(&InstKind) -> bool) -> usize {
        let cfg = Cfg::of(self);
        let loops = Loops::of(self, &cfg);
        let dominators = Dominators::of(self, &cfg);
        let mut moved = 0;
        for &block in loops.rpo() {
            let headers: Vec<_> = iter::successors(loops.innermost(block), |&h| loops.parent(h)).collect();
            let Some(&outermost) = headers.last() else { continue };
            if headers.iter().any(|&h| loops.is_irreducible(h)) {
                continue;
            }
            let Some(to) = dominators.idom(outermost) else { continue };
            let (go, stay): (Vec<Inst>, Vec<Inst>) =
                self.blocks[block].insts.iter().partition(|&&i| movable(&self.insts[i].kind));
            moved += go.len();
            self.blocks[block].insts = stay;
            self.blocks[to].insts.extend(go);
        }
        moved
    }
}
