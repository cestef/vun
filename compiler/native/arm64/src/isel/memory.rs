//! Memory: copies, through a register or in a loop; loads and stores are snippets, whose
//! addresses fold a slot's and constant offsets where a form reaches them, ordered ones `ldar`
//! and `stlr`, which together are sequentially consistent.

use vun_asm::{RelocKind, Via};
use vun_asm_arm64::{Cc, Emit, Mem, Op, Operand, Reg, Size, SysReg};
use vun_codegen::Arch;
use vun_lir::{Address, DataId, Value};

use super::Sel;
use crate::Arm;
use crate::asm::AsmExt;
use vun_native::select::Carries;

/// Copies up to this many bytes move through a register, larger ones in a loop.
pub(super) const UNROLLED: u64 = 128;
/// The bytes a loop's step copies.
const STEP: u64 = 8;

impl Sel<'_, '_> {
    /// Copies `bytes` bytes, through a register or, past `UNROLLED`, in a loop of words.
    /// The address of the calling thread's storage of `data`, in `x0`: its TLS descriptor's
    /// call gives its offset from the thread pointer. A linker writes the offset itself where it
    /// knows it, in an executable, which is why the sequence is the one it recognizes.
    pub(super) fn thread_addr(&mut self, data: DataId) {
        let target = Address::Data(data);
        let (address, call) = (Reg::X0, Reg::X1);
        self.asm.emit(Op::Adrp, Size::B64, &[address.into(), Operand::Page(target, Via::Descriptor)]);
        let entry = Mem::Lo12 { base: address, target, via: Via::Descriptor };
        self.asm.emit(Op::Ldr, Size::B64, &[call.into(), entry.into()]);
        self.asm.emit(
            Op::Add,
            Size::B64,
            &[address.into(), address.into(), Operand::Lo12(target, Via::Descriptor)],
        );
        let at = self.asm.here();
        self.asm.relocate(at, target, RelocKind::TlsDescCall, 0);
        self.asm.emit(Op::Blr, Size::B64, &[call.into()]);
        self.asm.emit(Op::Mrs, Size::B64, &[call.into(), Operand::Sys(SysReg::TpidrEl0)]);
        self.asm.emit(Op::Add, Size::B64, &[address.into(), call.into(), address.into()]);
    }

    pub(super) fn copy(&mut self, to: Value, from: Value, bytes: u64) {
        if bytes <= UNROLLED {
            let (to, from) = (self.reg(to), self.reg(from));
            Arm::copy_bytes(self.0, to, from, bytes, Arm::CARRIER);
            return;
        }
        let (dst, src, left) = (self.scratch(), self.scratch(), self.scratch());
        self.reg_to(to, dst);
        self.reg_to(from, src);
        let steps = i64::try_from(bytes / STEP).expect("copies fit memory");
        self.asm.constant(left, steps.cast_unsigned());
        let again = self.asm.label();
        self.asm.bind(again);
        let step = i64::try_from(STEP).expect("a word");
        let (read, written) = (Mem::Post { base: src, offset: step }, Mem::Post { base: dst, offset: step });
        self.asm.emit(Op::Ldr, Size::B64, &[Arm::CARRIER.into(), read.into()]);
        self.asm.emit(Op::Str, Size::B64, &[Arm::CARRIER.into(), written.into()]);
        self.asm.emit(Op::Subs, Size::B64, &[left.into(), left.into(), Operand::Imm(1)]);
        self.asm.emit(Op::Bcond, Size::B64, &[Cc::Ne.into(), again.into()]);
        Arm::copy_bytes(self.0, dst, src, bytes % STEP, Arm::CARRIER);
    }
}
