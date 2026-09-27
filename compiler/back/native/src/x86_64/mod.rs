//! x86-64: what the framework needs of the architecture (registers, its own moves), the operands
//! and forms snippets are encoded with, the System V convention, the snippets and the
//! instruction compilers.

pub mod abi;
mod isel;
mod snippets;

use vun_asm::Via;
use vun_asm::imm::low64;
use vun_asm_x86_64::{Asm, Cc, Emit, Mem, Op, Operand, Reg, Size, X86_64};
use vun_codegen::snippet::{Emits, Width};
use vun_codegen::{Anchor, Arch, Place, Recompute};
use vun_dwarf::CallFrame;
use vun_lir::{Address, Cond};
use vun_target::Convention;

/// x86-64, as the framework drives it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct X86;

/// An operand of an emitted instruction.
pub type Opnd = Operand<Address>;

impl Arch for X86 {
    type Isa = X86_64;
    type Reg = Reg;
    type State = isel::State;

    const REGS: usize = Reg::ALL.len();
    /// Those a call may overwrite, but the two kept for the framework's moves.
    const ALLOCATABLE: &'static [Reg] = &[Reg::Rax, Reg::Rcx, Reg::Rdx, Reg::Rsi, Reg::Rdi, Reg::R8, Reg::R9];
    const FRAME: Reg = Reg::Rbp;
    const CARRIER: Reg = Reg::R11;
    const TEMP: Reg = Reg::R10;
    const WORD: u64 = 8;
    const STACK_ALIGN: u64 = 16;

    fn index(r: Reg) -> usize {
        r.number() as usize
    }

    fn dwarf(r: Reg) -> u16 {
        DWARF[r.number() as usize]
    }

    fn copy(code: &mut Asm<Address>, dst: Reg, src: Reg) {
        if dst != src {
            code.emit(Op::Mov, Size::B64, &[dst.into(), src.into()]);
        }
    }

    fn load(code: &mut Asm<Address>, dst: Reg, offset: i32) {
        code.emit(Op::Mov, Size::B64, &[dst.into(), Mem::at(Reg::Rbp, offset).into()]);
    }

    fn store(code: &mut Asm<Address>, offset: i32, src: Reg) {
        code.emit(Op::Mov, Size::B64, &[Mem::at(Reg::Rbp, offset).into(), src.into()]);
    }

    fn recompute(code: &mut Asm<Address>, dst: Reg, what: Recompute) {
        match what {
            Recompute::Const(bits) => {
                code.emit(Op::Mov, Size::B64, &[dst.into(), Opnd::Imm(low64(bits).cast_signed())]);
            }
            Recompute::Addr(place) => {
                if let Some(mem) = memory(place) {
                    code.emit(Op::Lea, Size::B64, &[dst.into(), mem.into()]);
                } else {
                    // Something defined elsewhere, reached through the global offset table.
                    let Anchor::Sym(target) = place.anchor else { unreachable!("frames fit 2 GiB") };
                    code.emit(Op::Mov, Size::B64, &[dst.into(), Mem::rip(target, Via::Got).into()]);
                    if place.disp != 0 {
                        code.emit(Op::Add, Size::B64, &[dst.into(), Opnd::Imm(place.disp)]);
                    }
                }
            }
        }
    }

    fn jump(code: &mut Asm<Address>, to: vun_asm::Label) {
        code.emit(Op::Jmp, Size::B64, &[to.into()]);
    }
}

/// Memory at `place`, when one operand reaches it: an import's address is read from the
/// global offset table first.
fn memory(place: Place) -> Option<Mem<Address>> {
    let disp = i32::try_from(place.disp).ok()?;
    match place.anchor {
        Anchor::Frame => Some(Mem::at(Reg::Rbp, disp)),
        Anchor::Sym(a @ (Address::Func(_) | Address::Data(_))) => Mem::rip(a, Via::Direct).plus(place.disp),
        Anchor::Sym(Address::Import(_)) => None,
    }
}

impl Emits for X86 {
    type Op = Op;
    type Size = Size;
    type Operand = Opnd;
    type Cond = Cc;

    const MOV: Op = Op::Mov;
    const WHOLE: Size = Size::B64;

    fn reg(r: Reg) -> Opnd {
        r.into()
    }

    fn imm(value: i64) -> Opnd {
        Opnd::Imm(value)
    }

    fn cond(cond: Cc) -> Opnd {
        cond.into()
    }

    fn mem(place: Place) -> Option<Opnd> {
        memory(place).map(Opnd::Mem)
    }

    fn at(base: Reg) -> Opnd {
        Mem::at(base, 0).into()
    }

    /// Narrow values compute at 32 bits: the bits above a value's own are undefined in a register.
    fn size(width: Width<Size>, size: Size) -> Size {
        match width {
            Width::Op => size,
            Width::Word => size.max(Size::B32),
            Width::Fixed(s) => s,
        }
    }

    fn fits(op: Op, size: Size, operands: &[Opnd]) -> bool {
        op.form(size, operands).is_some()
    }

    fn implicit(op: Op) -> &'static [Reg] {
        op.implicit()
    }

    fn emit(code: &mut Asm<Address>, op: Op, size: Size, operands: &[Opnd]) {
        code.emit(op, size, operands);
    }
}

/// The condition an integer comparison holds on after `cmp a, b`; `None` for floats.
#[must_use]
pub const fn condition(cond: Cond) -> Option<Cc> {
    Some(match cond {
        Cond::Eq => Cc::Z,
        Cond::Ne => Cc::Nz,
        Cond::SLt => Cc::L,
        Cond::SLe => Cc::Le,
        Cond::SGt => Cc::G,
        Cond::SGe => Cc::Ge,
        Cond::ULt => Cc::B,
        Cond::ULe => Cc::Be,
        Cond::UGt => Cc::A,
        Cond::UGe => Cc::Ae,
        Cond::FEq | Cond::FNe | Cond::FLt | Cond::FLe | Cond::FGt | Cond::FGe => return None,
    })
}

/// The number the System V psABI gives each register in DWARF, by its encoding: `rdx` and `rcx`,
/// `rsi`, `rdi`, `rbp` and `rsp` are numbered in another order.
const DWARF: [u16; 16] = [0, 2, 1, 3, 7, 6, 4, 5, 8, 9, 10, 11, 12, 13, 14, 15];

/// System V is the one convention of x86-64 targets yet.
impl crate::Native for X86 {
    /// The call pushes the return address, `rip`'s column.
    const CALL_FRAME: CallFrame = CallFrame {
        code_align: 1,
        stack_pointer: 7,
        frame_pointer: 6,
        return_address: 16,
        return_pushed: true,
    };

    const CONVENTIONS: &'static [(Convention, Self::Convention)] = &[(Convention::Sysv, &abi::SYSV)];
}
