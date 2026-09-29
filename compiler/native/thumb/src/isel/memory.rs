//! Memory: copies, through a register or in a loop; loads and stores are snippets, whose
//! addresses fold a slot's and constant offsets where a form reaches them, ordered ones between
//! barriers.

use vun_asm_thumb::{Cc, Emit, Mem, Op, Operand, Size};
use vun_codegen::Arch;
use vun_lir::Value;

use super::Sel;
use crate::Thumb2;
use crate::asm::AsmExt;
use vun_native::select::Carries;
use vun_native::width::offset_of;

/// Copies up to this many bytes move through a register, larger ones in a loop.
pub(super) const UNROLLED: u64 = 64;
/// The bytes a loop's step copies: a word.
const STEP: u64 = Thumb2::WORD;

impl Sel<'_, '_> {
    /// Copies `bytes` bytes, through a register or, past `UNROLLED`, in a loop of words.
    pub(super) fn copy(&mut self, to: Value, from: Value, bytes: u64) {
        if bytes <= UNROLLED {
            let (to, from) = (self.reg(to), self.reg(from));
            Thumb2::copy_bytes(self.0, to, from, bytes, Thumb2::CARRIER);
            return;
        }
        let (dst, src, left) = (self.scratch(), self.scratch(), self.scratch());
        self.reg_to(to, dst);
        self.reg_to(from, src);
        self.asm.constant(left, bytes / STEP);
        let again = self.asm.label();
        self.asm.bind(again);
        let step = i64::from(offset_of(STEP));
        let (read, written) = (Mem::Post { base: src, offset: step }, Mem::Post { base: dst, offset: step });
        self.asm.emit(Op::Ldr, Size::B32, &[Thumb2::CARRIER.into(), read.into()]);
        self.asm.emit(Op::Str, Size::B32, &[Thumb2::CARRIER.into(), written.into()]);
        self.asm.emit(Op::Subs, Size::B32, &[left.into(), left.into(), Operand::Imm(1)]);
        self.asm.emit(Op::Bcond, Size::B32, &[Cc::Ne.into(), again.into()]);
        Thumb2::copy_bytes(self.0, dst, src, bytes % STEP, Thumb2::CARRIER);
    }
}
