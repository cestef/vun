//! Hoisting constants out of loops: a constant defined inside a loop is defined once before it, at the end of the block immediately dominating the header of the
//! outermost loop around it, so a backend keeping it in a register keeps it across the loop
//! rather than making it again each round. Irreducible loops, entered at more than one block,
//! are left as they are.

use std::iter;

use crate::analysis::{Cfg, Dominators, Loops};
use crate::entity::Inst;
use crate::func::Function;
use crate::inst::InstKind;

impl Function {
    /// Moves every constant out of the loops around it; returns how many moved.
    pub fn hoist(&mut self) -> usize {
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
            let (go, stay): (Vec<Inst>, Vec<Inst>) = self.blocks[block]
                .insts
                .iter()
                .partition(|&&i| matches!(self.insts[i].kind, InstKind::Const { .. }));
            moved += go.len();
            self.blocks[block].insts = stay;
            self.blocks[to].insts.extend(go);
        }
        moved
    }
}
