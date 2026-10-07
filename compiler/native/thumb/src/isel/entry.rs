//! The frame and the parameters: the prologue pushes the frame pointer and the link register,
//! reserves the frame, its size built in `r12` by two moves written when the function ends, and
//! saves `FAR`; parameters passed in memory are addresses recomputed where used, the others are
//! received where the convention passes them, and the plan moves them on from there.

use vun_asm_thumb::{Emit, List, Op, Operand, Reg, Size};
use vun_codegen::Arch;
use vun_dwarf::Step;
use vun_native::select::Params;

use super::Sel;
use crate::asm::AsmExt;
use crate::{FAR, FP, LR, Thumb2};

/// Where the caller's stack arguments start, above the saved frame pointer and link register.
pub(super) const CALLER_ARGS: i32 = 8;
/// The register the frame size is built in.
const SIZE: Reg = Thumb2::CARRIER;

/// The frame pointer and the link register, as the prologue pushes them; the epilogue pops the
/// return address into the program counter.
pub(super) const LINKAGE: List = List::Regs(1 << FP.number() | 1 << LR.number());
pub(super) const RETURN: List = List::Regs(1 << FP.number() | 1 << Reg::Pc.number());

impl Sel<'_, '_> {
    /// `push {r7, lr}; mov r7, sp; movw r12, -size; movt r12, -size; add sp, r12`, `FAR` saved,
    /// and where each parameter passed in memory is.
    pub(super) fn prologue(&mut self) {
        self.asm.emit(Op::Push, Size::B32, &[Operand::List(LINKAGE)]);
        self.unwound(Step::Pushed);
        self.asm.emit(Op::Mov, Size::B32, &[FP.into(), Reg::Sp.into()]);
        self.unwound(Step::Framed);
        self.state.frame_at = self.asm.here();
        self.asm.frame_size(SIZE, 0);
        self.asm.emit(Op::Add, Size::B32, &[Reg::Sp.into(), SIZE.into()]);
        let far = self.frame.reserve(Thumb2::WORD, Thumb2::WORD);
        Thumb2::store(&mut self.asm, far, FAR);
        self.state.far_at = far;
        let above = 2 * i64::try_from(Thumb2::WORD).expect("a word fits");
        self.unwound(Step::Saved { reg: Thumb2::dwarf(FAR), offset: i64::from(far) - above });
        Thumb2::place_params(self.0);
    }

    /// Writes the frame's size into the prologue's moves.
    pub(super) fn finish(&mut self, frame: u64) {
        let at = self.state.frame_at;
        self.asm.rewrite(at, |moves| moves.frame_size(SIZE, frame));
    }
}
