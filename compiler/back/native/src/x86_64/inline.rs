//! x86-64's table and operands for inline assembly: bindings in general registers (a shift's
//! count in `cl`), memory off a base register or a symbol this module defines.

use vun_asm::{Label, Untaken, Via};
use vun_asm_x86_64::{Base, Cc, Class, Emit, Form, Kind, Mem, Op, Operand, Reg, Size, Xmm};
use vun_backend::AsmOperand;
use vun_codegen::Code;
use vun_inline::{Disp, RegRef};
use vun_lir::Address;

use super::X86;
use crate::inline::{self, Inline, Inst};

/// The instruction `inst` is, and the form it was checked against.
fn entry(inst: &Inst) -> (Op, &'static Form) {
    let op = Op::ALL[usize::from(inst.op.0)];
    (op, &op.forms()[usize::from(inst.form.0)])
}

impl Inline for X86 {
    type Kind = Kind;
    type Operand = Operand<Address>;
    type Size = Size;

    fn kinds(inst: &Inst) -> impl Iterator<Item = Kind> {
        entry(inst).1.kinds()
    }

    fn implicit(inst: &Inst) -> impl Iterator<Item = Reg> {
        entry(inst).1.implicit.iter().copied()
    }

    fn general(r: RegRef) -> Option<Reg> {
        match Class::ALL[usize::from(r.class.0)] {
            Class::General => Some(Reg::ALL[usize::from(r.number)]),
            Class::Sse => None,
        }
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

    fn operand(
        operand: &inline::Operand,
        _: Kind,
        regs: &mut impl Iterator<Item = Reg>,
        labels: &[Label],
    ) -> Operand<Address> {
        let mut next = || regs.next().expect("a register for each binding");
        match operand {
            vun_inline::Operand::Reg(r) => {
                Self::general(*r).map_or(Operand::Xmm(Xmm(r.number)), Operand::Reg)
            }
            vun_inline::Operand::Imm(n) => Operand::Imm(*n),
            vun_inline::Operand::Cond(c) => Operand::Cond(Cc::ALL[usize::from(c.0)]),
            vun_inline::Operand::Label(line) => Operand::Label(labels[usize::from(line.0)]),
            vun_inline::Operand::Symbol(s) => Operand::Sym(*s),
            vun_inline::Operand::Bound(_) => Operand::Reg(next()),
            vun_inline::Operand::Mem(mem) => {
                let (base, disp) = match (&mem.base, mem.disp.clone()) {
                    (Some(vun_inline::Base::Reg(r)), Disp::Bytes(n)) => {
                        (Base::Reg(Self::general(*r).expect("a general base")), n)
                    }
                    (Some(vun_inline::Base::Bound(_)), Disp::Bytes(n)) => (Base::Reg(next()), n),
                    (None, Disp::Symbol(s, n)) => (Base::Rip(s, Via::Direct), n),
                    _ => unreachable!("an address x86-64 encodes"),
                };
                Operand::Mem(Mem { base, disp: i32::try_from(disp).expect("a displacement of 32 bits") })
            }
        }
    }

    fn emit(code: &mut Code<Self>, inst: &Inst, operands: &[Operand<Address>]) -> Result<(), Untaken> {
        let (op, form) = entry(inst);
        code.emit_by(op, form, Size::ALL[usize::from(inst.size.0)], operands)
    }

    fn load_at(code: &mut Code<Self>, r: Reg, addr: Reg, size: Size) {
        code.emit(Op::Mov, size, &[r.into(), Mem::at(addr, 0).into()]);
    }

    fn store_at(code: &mut Code<Self>, addr: Reg, r: Reg, size: Size) {
        code.emit(Op::Mov, size, &[Mem::at(addr, 0).into(), r.into()]);
    }
}
