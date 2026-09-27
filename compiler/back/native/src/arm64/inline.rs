//! arm64's table and operands for inline assembly: bindings in general registers, memory in
//! the addressing mode its form names, and symbols as branch targets, pages or their low bits.

use vun_asm::{Untaken, Via};
use vun_asm_arm64::{Emit, Kind, Mem, Mode, Op, Operand, Place, Reg, Size};
use vun_backend::AsmOperand;
use vun_codegen::Code;
use vun_inline::{Disp, Line, RegRef};
use vun_lir::Address;

use super::Arm;
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

impl Inline for Arm {
    type Kind = Kind;
    type Operand = Operand<Address>;
    type Size = Size;

    fn kinds(inst: &Inst) -> impl Iterator<Item = Kind> {
        Op::entry(inst.op, inst.form).1.args.iter().map(|arg| arg.kind)
    }

    fn implicit(inst: &Inst) -> impl Iterator<Item = Reg> {
        let form = Op::entry(inst.op, inst.form).1;
        form.reads.iter().chain(form.writes).copied()
    }

    fn general(r: RegRef) -> Option<Reg> {
        Reg::named(r)
    }

    fn fixed(kind: Option<Kind>, named: Option<Reg>) -> Result<Option<Reg>, AsmOperand> {
        match kind {
            Some(Kind::Gpr(..) | Kind::Shifted(_) | Kind::Extended) | None => Ok(named),
            Some(_) => Err(AsmOperand::Unallocated),
        }
    }

    fn addressing(mem: &inline::Mem, kind: Kind) -> Result<(), AsmOperand> {
        let Kind::Mem(mode, _) = kind else { return Err(AsmOperand::Unencodable) };
        match (mode, &mem.base, &mem.index, &mem.disp) {
            (_, None, _, _) => Err(AsmOperand::Absolute),
            (Mode::Index, Some(_), Some((_, 1)), Disp::Bytes(0))
            | (Mode::Lo12, Some(_), None, Disp::Symbol(_, 0))
            | (_, Some(_), None, Disp::Bytes(_)) => Ok(()),
            (Mode::Index | Mode::Lo12, ..) => Err(AsmOperand::Unencodable),
            (_, Some(_), Some(_), _) => Err(AsmOperand::Indexed),
            (_, Some(_), None, Disp::Symbol(..)) => Err(AsmOperand::SymbolOffRegister),
        }
    }

    fn operand(operand: &inline::Operand, kind: Kind, placed: &mut Placed<'_, Reg>) -> Operand<Address> {
        Operand::of(operand, kind, placed)
    }

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
