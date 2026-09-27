//! x86-64's table and operands for inline assembly: bindings in general registers (a shift's
//! count in `cl`), memory off a base register or a symbol this module defines.

use vun_asm::{Untaken, Via};
use vun_asm_x86_64::{Emit, Kind, Op, Operand, Place, Reg, Size};
use vun_backend::AsmOperand;
use vun_codegen::Code;
use vun_inline::{Disp, Line, RegRef};
use vun_lir::Address;

use super::X86;
use crate::inline::{self, Inline, Inst, Placed};

impl Place<Address> for Placed<'_, Reg> {
    fn bound(&mut self) -> Reg {
        self.take()
    }

    fn line(&self, line: Line) -> Operand<Address> {
        Operand::Label(self.label(line))
    }

    fn via(&self, symbol: Address) -> Via {
        inline::via(symbol)
    }
}

impl Inline for X86 {
    type Kind = Kind;
    type Operand = Operand<Address>;
    type Size = Size;

    fn kinds(inst: &Inst) -> impl Iterator<Item = Kind> {
        Op::entry(inst.op, inst.form).1.kinds()
    }

    fn implicit(inst: &Inst) -> impl Iterator<Item = Reg> {
        Op::entry(inst.op, inst.form).1.implicit.iter().copied()
    }

    fn general(r: RegRef) -> Option<Reg> {
        Reg::named(r)
    }

    fn fixed(kind: Option<Kind>, named: Option<Reg>) -> Result<Option<Reg>, AsmOperand> {
        match kind {
            Some(Kind::Cl) => Ok(Some(Reg::Rcx)),
            Some(Kind::R | Kind::Rn(_) | Kind::Rm | Kind::Rmn(_)) | None => Ok(named),
            Some(_) => Err(AsmOperand::Unallocated),
        }
    }

    fn addressing(mem: &inline::Mem, _: Kind) -> Result<(), AsmOperand> {
        let fits = |n: i64| i32::try_from(n).map(drop).map_err(|_| AsmOperand::Absolute);
        match (&mem.base, &mem.index, &mem.disp) {
            (_, Some(_), _) => Err(AsmOperand::Indexed),
            (Some(_), None, Disp::Symbol(..)) => Err(AsmOperand::SymbolOffRegister),
            (None, None, Disp::Bytes(_)) => Err(AsmOperand::Absolute),
            (None, None, Disp::Symbol(Address::Import(_), _)) => Err(AsmOperand::Imported),
            (Some(_), None, Disp::Bytes(n)) | (None, None, Disp::Symbol(_, n)) => fits(*n),
        }
    }

    fn operand(operand: &inline::Operand, _: Kind, placed: &mut Placed<'_, Reg>) -> Operand<Address> {
        Operand::of(operand, placed)
    }

    fn emit(code: &mut Code<Self>, inst: &Inst, operands: &[Operand<Address>]) -> Result<(), Untaken> {
        let (op, form) = Op::entry(inst.op, inst.form);
        code.emit_by(op, form, Size::of(inst.size), operands)
    }

    fn load_at(code: &mut Code<Self>, r: Reg, addr: Reg, size: Size) {
        code.emit(Op::Mov, size, &[r.into(), vun_asm_x86_64::Mem::at(addr, 0).into()]);
    }

    fn store_at(code: &mut Code<Self>, addr: Reg, r: Reg, size: Size) {
        code.emit(Op::Mov, size, &[vun_asm_x86_64::Mem::at(addr, 0).into(), r.into()]);
    }
}
