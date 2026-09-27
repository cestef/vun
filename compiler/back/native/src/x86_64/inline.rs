//! x86-64's inline assembly in native code: memory at an import is refused, as only its entry in
//! the global offset table holds its address; outputs are moved through `mov`. A checked write
//! calls the runtime between pushes of the flags and of every register a call may overwrite,
//! over a stack aligned from `rbx`, which the call keeps.

use vun_asm::{AsmOperand, Blocks, Untaken};
use vun_asm_x86_64::{Base, Cc, Emit, Kind, Mem, Op, Operand, Reg, Size, X86_64};
use vun_backend::Missing;
use vun_codegen::Code;
use vun_inline::Disp;
use vun_lir::{Address, AsmCheck};

use super::X86;
use crate::inline::{self, Inline, Inst};

/// The registers kept around a checked write: those a call may overwrite, and `rbx`, which the
/// stack pointer is kept in while the stack is aligned for the call.
const KEPT: [Reg; 10] =
    [Reg::Rax, Reg::Rcx, Reg::Rdx, Reg::Rsi, Reg::Rdi, Reg::R8, Reg::R9, Reg::R10, Reg::R11, Reg::Rbx];

/// The alignment of the stack at a call.
const ALIGN: i64 = -16;

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

    fn check_write(
        code: &mut Code<Self>,
        at: &Operand<Address>,
        bytes: u32,
        check: &AsmCheck,
    ) -> Result<(), Missing> {
        code.emit(Op::Pushf, Size::B64, &[]);
        for r in KEPT {
            code.emit(Op::Push, Size::B64, &[r.into()]);
        }
        // An address off the stack pointer is further from it by what was pushed.
        let pushed = i32::try_from((KEPT.len() + 1) * 8).expect("a few words");
        let at = match *at {
            Operand::Mem(Mem { base: Base::Reg(Reg::Rsp), disp }) => {
                Operand::Mem(Mem { base: Base::Reg(Reg::Rsp), disp: disp + pushed })
            }
            other => other,
        };
        code.emit(Op::Lea, Size::B64, &[Reg::Rdi.into(), at]);
        code.emit(Op::Mov, Size::B32, &[Reg::Rsi.into(), Operand::Imm(i64::from(bytes))]);
        code.emit(Op::Mov, Size::B32, &[Reg::Rdx.into(), Operand::Imm(i64::from(check.site))]);
        code.emit(Op::Mov, Size::B64, &[Reg::Rbx.into(), Reg::Rsp.into()]);
        code.emit(Op::And, Size::B64, &[Reg::Rsp.into(), Operand::Imm(ALIGN)]);
        code.emit(Op::Call, Size::B64, &[Operand::Sym(check.write)]);
        code.emit(Op::Mov, Size::B64, &[Reg::Rsp.into(), Reg::Rbx.into()]);
        for r in KEPT.into_iter().rev() {
            code.emit(Op::Pop, Size::B64, &[r.into()]);
        }
        code.emit(Op::Popf, Size::B64, &[]);
        Ok(())
    }

    fn stack(code: &mut Code<Self>, dst: Reg) {
        code.emit(Op::Mov, Size::B64, &[dst.into(), Reg::Rsp.into()]);
    }

    /// The arguments are moved so that none is overwritten before it is read.
    fn check_stack(code: &mut Code<Self>, before: Reg, check: &AsmCheck) {
        let kept = code.label();
        code.emit(Op::Cmp, Size::B64, &[Reg::Rsp.into(), before.into()]);
        code.emit(Op::J, Size::B64, &[Cc::Z.into(), kept.into()]);
        code.emit(Op::Mov, Size::B64, &[Reg::Rsi.into(), before.into()]);
        code.emit(Op::Mov, Size::B64, &[Reg::Rdx.into(), Reg::Rsp.into()]);
        code.emit(Op::Mov, Size::B32, &[Reg::Rdi.into(), Operand::Imm(i64::from(check.site))]);
        code.emit(Op::And, Size::B64, &[Reg::Rsp.into(), Operand::Imm(ALIGN)]);
        code.emit(Op::Call, Size::B64, &[Operand::Sym(check.stack)]);
        code.bind(kept);
    }

    fn load_at(code: &mut Code<Self>, r: Reg, addr: Reg, size: Size) {
        code.emit(Op::Mov, size, &[r.into(), Mem::at(addr, 0).into()]);
    }

    fn store_at(code: &mut Code<Self>, addr: Reg, r: Reg, size: Size) {
        code.emit(Op::Mov, size, &[Mem::at(addr, 0).into(), r.into()]);
    }
}
