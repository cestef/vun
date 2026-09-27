//! Memory: copies, through a register or in a loop; loads and stores are snippets, whose
//! addresses fold a slot's and constant offsets where a form reaches them, ordered ones `ldar`
//! and `stlr`, which together are sequentially consistent.

use vun_asm_arm64::{Cc, Emit, Mem, Op, Operand, Size};
use vun_codegen::Arch;
use vun_lir::Value;

use super::Sel;
use crate::arm64::Arm;

/// Copies up to this many bytes move through a register, larger ones in a loop.
pub(super) const UNROLLED: u64 = 128;
/// The bytes a loop's step copies.
const STEP: u64 = 8;

impl Sel<'_, '_> {
    /// Copies `bytes` bytes, through a register or, past `UNROLLED`, in a loop of words.
    pub(super) fn copy(&mut self, to: Value, from: Value, bytes: u64) {
        if bytes <= UNROLLED {
            let (to, from) = (self.reg(to), self.reg(from));
            self.copy_bytes(to, from, bytes, Arm::CARRIER);
            return;
        }
        let (dst, src, left) = (self.scratch(), self.scratch(), self.scratch());
        self.reg_to(to, dst);
        self.reg_to(from, src);
        let steps = i64::try_from(bytes / STEP).expect("copies fit memory");
        crate::arm64::constant(&mut self.asm, left, steps.cast_unsigned());
        let again = self.asm.label();
        self.asm.bind(again);
        let step = i64::try_from(STEP).expect("a word");
        let (read, written) = (Mem::Post { base: src, offset: step }, Mem::Post { base: dst, offset: step });
        self.asm.emit(Op::Ldr, Size::B64, &[Arm::CARRIER.into(), read.into()]);
        self.asm.emit(Op::Str, Size::B64, &[Arm::CARRIER.into(), written.into()]);
        self.asm.emit(Op::Subs, Size::B64, &[left.into(), left.into(), Operand::Imm(1)]);
        self.asm.emit(Op::Bcond, Size::B64, &[Cc::Ne.into(), again.into()]);
        self.copy_bytes(dst, src, bytes % STEP, Arm::CARRIER);
    }
}
