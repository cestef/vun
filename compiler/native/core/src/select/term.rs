//! Terminators on load and store architectures: a branch jumps on the flags of the comparison it
//! fuses, or tests its condition; a switch compares case by case; a return passes the result
//! where the convention returns it, then leaves the frame. Each jump runs the plan's moves for
//! its edge (`Cx::jump`), a conditional one on its own way to its target. What differs between
//! architectures, the instructions, are the hooks of [`Terminates`].

use vun_backend::Lack;
use vun_codegen::snippet::{Args, Emits};
use vun_codegen::{Cx, Layout};
use vun_lir::{Block, Cond, Target, Terminator};

use super::{Frames, Params};

/// What a load and store architecture's terminators are written with.
pub trait Terminates: Frames {
    /// Where two values compared equal.
    const EQUAL: <Self as Emits>::Cond;

    /// The condition the flags hold on after the comparison `cond`, where one does.
    fn holds(cond: Cond) -> Option<<Self as Emits>::Cond>;

    /// The condition holding where `cond` does not.
    fn inverse(cond: <Self as Emits>::Cond) -> <Self as Emits>::Cond;

    /// Tests the boolean in `r`, giving the condition holding where it is true.
    fn test(cx: &mut Cx<'_, Self>, r: Self::Reg) -> <Self as Emits>::Cond;

    /// The register a switch compares its value in, extended to the width it compares at, and
    /// that width.
    ///
    /// # Errors
    /// What the backend lacks for the value.
    fn switched(cx: &mut Cx<'_, Self>, v: vun_lir::Value)
    -> Result<(Self::Reg, <Self as Emits>::Size), Lack>;

    /// Compares `r`, of `size`, with the case `bits`.
    fn compare_case(cx: &mut Cx<'_, Self>, r: Self::Reg, size: <Self as Emits>::Size, bits: u128);

    /// Branches to `label` where `cond` holds.
    fn branch_if(cx: &mut Cx<'_, Self>, cond: <Self as Emits>::Cond, label: vun_asm::Label);

    /// Stops the program: code no path reaches.
    fn trap(cx: &mut Cx<'_, Self>);

    /// Leaves the frame and returns.
    fn epilogue(cx: &mut Cx<'_, Self>);

    /// The terminator of `block`.
    ///
    /// # Errors
    /// What the backend lacks for it.
    fn close(cx: &mut Cx<'_, Self>, block: Block) -> Result<(), Lack> {
        let f = cx.f;
        let term = &f.blocks[block].term;
        // A fused comparison sets the flags here; the stores and moves up to the jump keep them.
        let flags = match cx.fused(block) {
            Some(fused) => {
                let (snippet, size) = Self::flags(cx, fused.cond, fused.a)?;
                cx.snippet(snippet, &Args { ins: &[fused.a, fused.b], out: None, size, cond: None });
                Some(Self::holds(fused.cond).expect("only comparisons on one condition fuse"))
            }
            None => None,
        };
        let next = cx.next(block);
        match term {
            Terminator::Jump(target) => cx.jump(target, next),
            Terminator::Branch { cond, then, otherwise } => {
                let holds = if let Some(holds) = flags {
                    holds
                } else {
                    let r = cx.reg(*cond);
                    Self::test(cx, r)
                };
                let layout = Layout::of(then, otherwise, next);
                let holds = if layout.inverted { Self::inverse(holds) } else { holds };
                Self::jump_if(cx, holds, layout.taken);
                cx.jump(layout.fall, next);
            }
            Terminator::Switch { value, cases, default } => {
                let (r, size) = Self::switched(cx, *value)?;
                for &(bits, ref target) in cases {
                    Self::compare_case(cx, r, size, bits);
                    Self::jump_if(cx, Self::EQUAL, target);
                }
                cx.jump(default, next);
            }
            Terminator::Return(value) => {
                Self::returned(cx, *value)?;
                Self::epilogue(cx);
            }
            Terminator::Unreachable => Self::trap(cx),
        }
        Ok(())
    }

    /// Jumps to `target` where `holds` does, running the moves of its edge only on the way there.
    fn jump_if(cx: &mut Cx<'_, Self>, holds: <Self as Emits>::Cond, target: &Target) {
        if cx.moves_on(target) {
            let skip = cx.asm.label();
            Self::branch_if(cx, Self::inverse(holds), skip);
            cx.jump(target, None);
            cx.asm.bind(skip);
        } else {
            let label = cx.label(target.block);
            Self::branch_if(cx, holds, label);
        }
    }
}
