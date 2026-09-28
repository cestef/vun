//! arm64: what the framework needs of the architecture (registers, its own moves), the operands
//! and forms snippets are encoded with, AAPCS64, the snippets and the instruction compilers.
//! Homes are reached from the frame pointer, `x29`; `x16` and `x17` are the framework's own, and
//! `x15` holds offsets and addresses too far for one instruction.

pub mod abi;
mod asm;
mod inline;
mod isel;
mod snippets;
#[cfg(test)]
mod tests;

use self::asm::AsmExt;
use vun_asm_arm64::{Arm64, Asm, Cc, Emit, Mem, Op, Operand, Reg, Size};
use vun_codegen::snippet::{Emits, Width};
use vun_codegen::{Anchor, Arch, Place, Recompute};
use vun_dwarf::CallFrame;
use vun_lir::{Address, Cond};
use vun_target::Convention;

/// arm64, as the framework drives it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Arm;

/// An operand of an emitted instruction.
pub type Opnd = Operand<Address>;

/// The frame pointer, which homes are reached from.
pub const FP: Reg = Reg::X29;
/// The link register.
pub const LR: Reg = Reg::X30;
/// The register an offset or an address too far for one instruction is built in, apart from any
/// value and from the framework's own.
pub const FAR: Reg = Reg::X15;

impl Arch for Arm {
    type Isa = Arm64;
    type Reg = Reg;
    type State = isel::State;

    const REGS: usize = Reg::Zr as usize + 1;
    /// Those a call may overwrite, but the ones kept for the framework and `FAR`.
    const ALLOCATABLE: &'static [Reg] = &[
        Reg::X0,
        Reg::X1,
        Reg::X2,
        Reg::X3,
        Reg::X4,
        Reg::X5,
        Reg::X6,
        Reg::X7,
        Reg::X8,
        Reg::X9,
        Reg::X10,
        Reg::X11,
        Reg::X12,
        Reg::X13,
        Reg::X14,
    ];
    const FRAME: Reg = FP;
    const CARRIER: Reg = Reg::X16;
    const TEMP: Reg = Reg::X17;
    const WORD: u64 = 8;
    const STACK_ALIGN: u64 = 16;

    fn index(r: Reg) -> usize {
        r as usize
    }

    /// `xN` is `N`, the stack pointer 31.
    fn dwarf(r: Reg) -> u16 {
        r as u16
    }

    fn copy(code: &mut Asm<Address>, dst: Reg, src: Reg) {
        if dst != src {
            code.emit(Op::Mov, Size::B64, &[dst.into(), src.into()]);
        }
    }

    fn load(code: &mut Asm<Address>, dst: Reg, offset: i32) {
        code.access(Op::Ldr, Size::B64, dst.into(), FP, i64::from(offset), dst);
    }

    fn store(code: &mut Asm<Address>, offset: i32, src: Reg) {
        code.access(Op::Str, Size::B64, src.into(), FP, i64::from(offset), FAR);
    }

    /// A constant one move takes, and an address in the frame; a symbol's takes two
    /// instructions.
    fn in_place(what: Recompute, bits: u32) -> bool {
        match what {
            Recompute::Const(c) => {
                let v = vun_asm::imm::sign_extend(vun_asm::imm::low64(c), bits);
                Op::Mov.form(Size::B64, &[Opnd::Reg(FAR), Opnd::Imm(v)]).is_some()
            }
            Recompute::Addr(Place { anchor, .. }) => anchor == Anchor::Frame,
        }
    }

    fn recompute(code: &mut Asm<Address>, dst: Reg, what: Recompute) {
        match what {
            Recompute::Const(bits) => {
                code.constant(dst, vun_asm::imm::low64(bits));
            }
            Recompute::Addr(Place { anchor: Anchor::Frame, disp }) => code.offset(dst, FP, disp),
            Recompute::Addr(Place { anchor: Anchor::Sym(target), disp }) => {
                code.address(dst, target);
                code.offset(dst, dst, disp);
            }
        }
    }

    fn jump(code: &mut Asm<Address>, to: vun_asm::Label) {
        code.emit(Op::B, Size::B64, &[to.into()]);
    }
}

impl Emits for Arm {
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

    /// Memory in the frame; what the linker places is reached through its address in a register.
    fn mem(place: Place) -> Option<Opnd> {
        match place.anchor {
            Anchor::Frame => Some(Mem::at(FP, place.disp).into()),
            Anchor::Sym(_) => None,
        }
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

    fn implicit(_: Op) -> &'static [Reg] {
        &[]
    }

    fn emit(code: &mut Asm<Address>, op: Op, size: Size, operands: &[Opnd]) {
        code.emit(op, size, operands);
    }
}

/// The condition a comparison holds on after `cmp a, b` or `fcmp a, b`; `None` for ordered
/// inequality of floats, which is two conditions. An unordered `fcmp` sets carry and overflow
/// only, which none of these holds on.
#[must_use]
pub const fn condition(cond: Cond) -> Option<Cc> {
    Some(match cond {
        Cond::Eq | Cond::FEq => Cc::Eq,
        Cond::Ne => Cc::Ne,
        Cond::SLt => Cc::Lt,
        Cond::SLe => Cc::Le,
        Cond::SGt | Cond::FGt => Cc::Gt,
        Cond::SGe | Cond::FGe => Cc::Ge,
        Cond::ULt => Cc::Lo,
        Cond::ULe | Cond::FLe => Cc::Ls,
        Cond::UGt => Cc::Hi,
        Cond::UGe => Cc::Hs,
        Cond::FLt => Cc::Mi,
        Cond::FNe => return None,
    })
}

impl vun_native::Native for Arm {
    /// Instructions are 4 bytes; the return address is in the link register, `x30`.
    const CALL_FRAME: CallFrame = CallFrame {
        code_align: 4,
        stack_pointer: 31,
        frame_pointer: 29,
        return_address: 30,
        return_pushed: false,
    };

    const CONVENTIONS: &'static [(Convention, Self::Convention)] = vun_asm_arm64::conventions::CONVENTIONS;
}
