//! arm64's table and operands for inline assembly: bindings in general registers, memory in
//! the addressing mode its form names, and symbols as branch targets, pages or their low bits.

use vun_asm::{Label, Untaken, Via};
use vun_asm_arm64::{Cc, Class, Emit, Extend, Form, Kind, Mem, Mode, Op, Operand, Reg, Scale, Size, V};
use vun_backend::AsmOperand;
use vun_codegen::Code;
use vun_inline::{Disp, RegRef};
use vun_lir::{Address, Output, Value};

use super::Arm;
use crate::inline::{self, Inline, Inst};

/// The instruction `inst` is, and the form it was checked against.
fn entry(inst: &Inst) -> (Op, &'static Form) {
    let op = Op::ALL[usize::from(inst.op.0)];
    (op, &op.forms()[usize::from(inst.form.0)])
}

/// How an operand reaches `target`: an import through its entry in the global offset table.
fn via(target: Address) -> Via {
    match target {
        Address::Import(_) => Via::Got,
        Address::Func(_) | Address::Data(_) => Via::Direct,
    }
}

/// The register of an address: the one it names, or the next binding's.
fn register(base: &vun_inline::Base<Value, Output>, regs: &mut impl Iterator<Item = Reg>) -> Reg {
    match base {
        vun_inline::Base::Reg(r) => Arm::general(*r).expect("a general register of an address"),
        vun_inline::Base::Bound(_) => regs.next().expect("a register for each binding"),
    }
}

impl Inline for Arm {
    type Kind = Kind;
    type Operand = Operand<Address>;
    type Size = Size;

    fn kinds(inst: &Inst) -> impl Iterator<Item = Kind> {
        entry(inst).1.args.iter().map(|arg| arg.kind)
    }

    fn implicit(inst: &Inst) -> impl Iterator<Item = Reg> {
        let form = entry(inst).1;
        form.reads.iter().chain(form.writes).copied()
    }

    fn general(r: RegRef) -> Option<Reg> {
        match Class::ALL[usize::from(r.class.0)] {
            Class::General => Some(Reg::ALL[usize::from(r.number)]),
            Class::Fp => None,
        }
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

    fn operand(
        operand: &inline::Operand,
        kind: Kind,
        regs: &mut impl Iterator<Item = Reg>,
        labels: &[Label],
    ) -> Operand<Address> {
        match operand {
            vun_inline::Operand::Reg(r) => Self::general(*r).map_or(Operand::V(V(r.number)), Operand::Reg),
            vun_inline::Operand::Imm(n) => Operand::Imm(*n),
            vun_inline::Operand::Cond(c) => Operand::Cond(Cc::ALL[usize::from(c.0)]),
            vun_inline::Operand::Label(line) => Operand::Label(labels[usize::from(line.0)]),
            vun_inline::Operand::Symbol(s) => match kind {
                Kind::Page => Operand::Page(*s, via(*s)),
                Kind::Lo12 => Operand::Lo12(*s),
                _ => Operand::Sym(*s),
            },
            vun_inline::Operand::Bound(_) => Operand::Reg(regs.next().expect("a register for each binding")),
            vun_inline::Operand::Mem(mem) => {
                let base = register(mem.base.as_ref().expect("an address with a base"), regs);
                let Kind::Mem(mode, _) = kind else { unreachable!("memory where the form takes it") };
                Operand::Mem(match (mode, &mem.disp, &mem.index) {
                    (Mode::Index, _, Some((index, _))) => Mem::Index {
                        base,
                        index: register(index, regs),
                        extend: Extend::LSL,
                        scale: Scale::One,
                    },
                    (Mode::Lo12, &Disp::Symbol(target, _), _) => Mem::Lo12 { base, target, via: via(target) },
                    (Mode::Pre | Mode::PairPre, &Disp::Bytes(offset), _) => Mem::Pre { base, offset },
                    (Mode::Post | Mode::PairPost, &Disp::Bytes(offset), _) => Mem::Post { base, offset },
                    (_, &Disp::Bytes(offset), _) => Mem::Off { base, offset },
                    _ => unreachable!("an address arm64 encodes"),
                })
            }
        }
    }

    fn emit(code: &mut Code<Self>, inst: &Inst, operands: &[Operand<Address>]) -> Result<(), Untaken> {
        let (op, form) = entry(inst);
        code.emit_by(op, form, Size::ALL[usize::from(inst.size.0)], operands)
    }

    fn load_at(code: &mut Code<Self>, r: Reg, addr: Reg, size: Size) {
        code.emit(Op::Ldr, size, &[r.into(), Mem::at(addr, 0).into()]);
    }

    fn store_at(code: &mut Code<Self>, addr: Reg, r: Reg, size: Size) {
        code.emit(Op::Str, size, &[r.into(), Mem::at(addr, 0).into()]);
    }
}
