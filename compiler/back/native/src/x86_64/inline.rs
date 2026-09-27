//! x86-64's inline assembly in native code: memory at an import is refused, as only its entry in
//! the global offset table holds its address; outputs are moved through `mov`.

use vun_asm::{AsmOperand, Blocks, Untaken};
use vun_asm_x86_64::{Emit, Kind, Mem, Op, Operand, Reg, Size, X86_64};
use vun_codegen::Code;
use vun_inline::Disp;
use vun_lir::Address;

use super::X86;
use crate::inline::{self, Inline, Inst};

impl Inline for X86 {
    type Table = X86_64;
    type Size = Size;

    fn addressing(mem: &inline::Mem, kind: Kind) -> Result<(), AsmOperand> {
        if let (None, Disp::Symbol(Address::Import(_), _)) = (&mem.base, &mem.disp) {
            return Err(AsmOperand::Imported);
        }
        X86_64::addressable(mem, kind)
    }

    fn emit(code: &mut Code<Self>, inst: &Inst, operands: &[Operand<Address>]) -> Result<(), Untaken> {
        let (op, form) = Op::entry(inst.op, inst.form);
        code.emit_by(op, form, Size::of(inst.size), operands)
    }

    fn load_at(code: &mut Code<Self>, r: Reg, addr: Reg, size: Size) {
        code.emit(Op::Mov, size, &[r.into(), Mem::at(addr, 0).into()]);
    }

    fn store_at(code: &mut Code<Self>, addr: Reg, r: Reg, size: Size) {
        code.emit(Op::Mov, size, &[Mem::at(addr, 0).into(), r.into()]);
    }
}
