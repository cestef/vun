//! Leaving a block: each jump runs the plan's moves for its edge, which leave every value its
//! target reads where the plan has it there, and fill the target's parameters. A comparison the
//! branch ending its block is the one use of is made by the branch.

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
        (here && self.values[value].uses == 1 && A::fuses(cond)).then_some(Fused { value, cond, a, b })
    }
}

impl<A: Arch> Cx<'_, A> {
    /// The block emitted after `block`, which it falls into without a jump.
    #[must_use]
    pub fn next(&self, block: Block) -> Option<Block> {
        self.order.next(block)
    }

    /// Whether the jump to `target` moves anything on its way, so that a conditional one goes
    /// there through its moves.
    #[must_use]
    pub fn moves_on(&self, target: &Target) -> bool {
        !self.plan().edge(self.block, target.block).is_empty()
    }

    /// Runs the moves of the edge to `target`, then jumps unless it is `next`.
    pub fn jump(&mut self, target: &Target, next: Option<Block>) {
        let moves = self.plan().edge(self.block, target.block).to_vec();
        self.run(&moves);
        if next != Some(target.block) {
            A::jump(&mut self.asm, self.labels[target.block]);
        }
    }
}
