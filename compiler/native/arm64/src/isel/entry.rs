//! The frame and the parameters: the prologue saves the frame pointer and the link register and
//! reserves the frame, its size built in `x16` by two moves written when the function ends;
//! parameters passed in memory are addresses recomputed where used, the others are received
//! where the convention passes them, and the plan moves them on from there.

use vun_asm_arm64::{Emit, Mem, Op, Reg, Size};
use vun_codegen::Arch;
use vun_dwarf::Step;
use vun_native::select::Frames;

use super::Sel;
use crate::asm::AsmExt;
use crate::{Arm, FP, LR};

/// Where the caller's stack arguments start, above the saved frame pointer and link register.
pub(super) const CALLER_ARGS: i32 = 16;
/// The bytes the prologue pushes: the frame pointer and the link register.
const PAIR: i64 = 16;
/// The register the frame size is built in.
const SIZE: Reg = Arm::CARRIER;

impl Sel<'_, '_> {
    /// `stp x29, x30, [sp, #-16]!; mov x29, sp; mov x16, size; sub sp, sp, x16`, and where each
    /// parameter passed in memory is.
    pub(super) fn prologue(&mut self) {
        let pushed = Mem::Pre { base: Reg::Sp, offset: -PAIR };
        self.asm.emit(Op::Stp, Size::B64, &[FP.into(), LR.into(), pushed.into()]);
        self.unwound(Step::Pushed);
        self.asm.emit(Op::Mov, Size::B64, &[FP.into(), Reg::Sp.into()]);
        self.unwound(Step::Framed);
        self.state.frame_at = self.asm.here();
        self.asm.frame_size(SIZE, 0);
        self.asm.emit(Op::Sub, Size::B64, &[Reg::Sp.into(), Reg::Sp.into(), SIZE.into()]);
        Arm::place_params(self.0);
    }

    /// Writes the frame's size into the prologue's moves.
    pub(super) fn finish(&mut self, frame: u64) {
        let at = self.state.frame_at;
        self.asm.rewrite(at, |moves| moves.frame_size(SIZE, frame));
    }
}
