//! arm64's inline assembly in native code: outputs are moved through `ldr` and `str`. A checked
//! block has its stack pointer compared; a checked write is refused until the flags can be kept
//! around the call (`mrs`, `msr` of `nzcv`).

use vun_asm::Untaken;
use vun_asm_arm64::{Arm64, Cc, Emit, Mem, Op, Operand, Reg, Size};
use vun_backend::Missing;
use vun_codegen::Arch;
use vun_codegen::Code;
use vun_lir::{Address, AsmCheck};

use super::Arm;
use super::asm::AsmExt;
use crate::inline::{Inline, Inst};

impl Inline for Arm {
    type Table = Arm64;
    type Size = Size;

    fn emit(code: &mut Code<Self>, inst: &Inst, operands: &[Operand<Address>]) -> Result<(), Untaken> {
        let (op, form) = Op::entry(inst.op, inst.form);
        code.emit_by(op, form, Size::of(inst.size), operands)
    }

    fn check_write(_: &mut Code<Self>, _: &Operand<Address>, _: u32, _: &AsmCheck) -> Result<(), Missing> {
        Err(Missing::AsmWriteCheck)
    }

    fn stack(code: &mut Code<Self>, dst: Reg) {
        code.emit(Op::Mov, Size::B64, &[dst.into(), Reg::Sp.into()]);
    }

    /// The stack pointer is read into the framework's own register, which compares take; the
    /// arguments are moved so that none is overwritten before it is read.
    fn check_stack(code: &mut Code<Self>, before: Reg, check: &AsmCheck) {
        let kept = code.label();
        code.emit(Op::Mov, Size::B64, &[Arm::TEMP.into(), Reg::Sp.into()]);
        code.emit(Op::Cmp, Size::B64, &[Arm::TEMP.into(), before.into()]);
        code.emit(Op::Bcond, Size::B64, &[Cc::Eq.into(), kept.into()]);
        code.emit(Op::Mov, Size::B64, &[Reg::X1.into(), before.into()]);
        code.emit(Op::Mov, Size::B64, &[Reg::X2.into(), Arm::TEMP.into()]);
        code.constant(Reg::X0, u64::from(check.site));
        code.emit(Op::Bl, Size::B64, &[Operand::Sym(check.stack)]);
        code.bind(kept);
    }

    fn load_at(code: &mut Code<Self>, r: Reg, addr: Reg, size: Size) {
        code.emit(Op::Ldr, size, &[r.into(), Mem::at(addr, 0).into()]);
    }

    fn store_at(code: &mut Code<Self>, addr: Reg, r: Reg, size: Size) {
        code.emit(Op::Str, size, &[r.into(), Mem::at(addr, 0).into()]);
    }
}
