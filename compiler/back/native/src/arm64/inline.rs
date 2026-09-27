//! arm64's inline assembly in native code: outputs are moved through `ldr` and `str`.

use vun_asm::Untaken;
use vun_asm_arm64::{Arm64, Emit, Mem, Op, Operand, Reg, Size};
use vun_codegen::Code;
use vun_lir::Address;

use super::Arm;
use crate::inline::{Inline, Inst};

impl Inline for Arm {
    type Table = Arm64;
    type Size = Size;

    fn emit(code: &mut Code<Self>, inst: &Inst, operands: &[Operand<Address>]) -> Result<(), Untaken> {
        let (op, form) = Op::entry(inst.op, inst.form);
        code.emit_by(op, form, Size::of(inst.size), operands)
    }

    fn load_at(code: &mut Code<Self>, r: Reg, addr: Reg, size: Size) {
        code.emit(Op::Ldr, size, &[r.into(), Mem::at(addr, 0).into()]);
    }

    fn store_at(code: &mut Code<Self>, addr: Reg, r: Reg, size: Size) {
        code.emit(Op::Str, size, &[r.into(), Mem::at(addr, 0).into()]);
    }
}
