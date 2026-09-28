//! Thumb-2, as Armv7-M runs it and Armv7-A in its Thumb state: what the framework needs of the
//! architecture (registers, its own moves), the operands and forms snippets are encoded with,
//! AAPCS, the snippets and the instruction compilers. Homes are reached from the frame pointer,
//! `r7`, as Thumb code keeps it; `r12` and `lr` are the framework's own, and `r11` builds offsets
//! too far for one instruction, saved by every prologue. Floats are single precision, computed by
//! the floating-point extension where the convention passes them in its registers.

pub mod abi;
mod asm;
mod inline;
mod isel;
mod snippets;
#[cfg(test)]
mod tests;

use self::asm::AsmExt;
use vun_asm_thumb::{Asm, Cc, Emit, Mem, Op, Operand, Reg, Size};
use vun_codegen::snippet::{Emits, Slot, Step, Width};
use vun_codegen::{Anchor, Arch, Place, Recompute};
use vun_dwarf::CallFrame;
use vun_lir::{Address, Cond};
use vun_object::{Float, Machine};
use vun_target::{Convention, TargetSpec};

/// Thumb-2, as the framework drives it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Thumb2;

/// An operand of an emitted instruction.
pub type Opnd = Operand<Address>;

/// The frame pointer, which homes are reached from.
pub const FP: Reg = Reg::R7;
/// The link register.
pub const LR: Reg = Reg::Lr;
/// The register an offset too far for one instruction is built in, apart from any value and from
/// the framework's own.
pub const FAR: Reg = Reg::R11;

impl Arch for Thumb2 {
    type Isa = vun_asm_thumb::Thumb;
    type Reg = Reg;
    type State = isel::State;

    const REGS: usize = Reg::Pc as usize + 1;
    /// Those a call may overwrite, but the ones kept for the framework.
    const ALLOCATABLE: &'static [Reg] = &[Reg::R0, Reg::R1, Reg::R2, Reg::R3];
    const FRAME: Reg = FP;
    const CARRIER: Reg = Reg::R12;
    const TEMP: Reg = LR;
    const WORD: u64 = 4;
    const STACK_ALIGN: u64 = 8;

    fn index(r: Reg) -> usize {
        r as usize
    }

    /// `rN` is `N`.
    fn dwarf(r: Reg) -> u16 {
        u16::from(r.number())
    }

    fn copy(code: &mut Asm<Address>, dst: Reg, src: Reg) {
        if dst != src {
            code.emit(Op::Mov, Size::B32, &[dst.into(), src.into()]);
        }
    }

    fn load(code: &mut Asm<Address>, dst: Reg, offset: i32) {
        code.access(Op::Ldr, Size::B32, dst.into(), FP, i64::from(offset), dst);
    }

    fn store(code: &mut Asm<Address>, offset: i32, src: Reg) {
        code.access(Op::Str, Size::B32, src.into(), FP, i64::from(offset), FAR);
    }

    /// A constant one move takes, and an address in the frame; a symbol's takes two
    /// instructions.
    fn in_place(what: Recompute, bits: u32) -> bool {
        match what {
            Recompute::Const(c) => {
                let v = vun_asm::imm::sign_extend(vun_asm::imm::low64(c), bits);
                Op::Mov.form(Size::B32, &[Opnd::Reg(FAR), Opnd::Imm(v)]).is_some()
            }
            Recompute::Addr(Place { anchor, .. }) => anchor == Anchor::Frame,
        }
    }

    fn recompute(code: &mut Asm<Address>, dst: Reg, what: Recompute) {
        match what {
            Recompute::Const(bits) => code.constant(dst, vun_asm::imm::low64(bits)),
            Recompute::Addr(Place { anchor: Anchor::Frame, disp }) => code.offset(dst, FP, disp),
            Recompute::Addr(Place { anchor: Anchor::Sym(target), disp }) => {
                code.address(dst, target);
                code.offset(dst, dst, disp);
            }
        }
    }

    fn jump(code: &mut Asm<Address>, to: vun_asm::Label) {
        code.emit(Op::B, Size::B32, &[to.into()]);
    }
}

/// An instruction of a snippet: one of the table's; one of those an access or an extension is,
/// by the width it runs at; or one an `it` block makes conditional, which the encoder emits as it
/// is, never omitting a move or bringing an operand into a register first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Instr {
    Is(Op),
    Load,
    Store,
    Zext,
    Sext,
    Then(Op),
}

impl Instr {
    /// The instruction it is at `size`, and the size that instruction runs at.
    pub(crate) const fn at(self, size: Size) -> (Op, Size) {
        match (self, size) {
            (Self::Is(op) | Self::Then(op), _) => (op, size),
            (Self::Load, Size::B8) => (Op::Ldrb, size),
            (Self::Load, Size::B16) => (Op::Ldrh, size),
            (Self::Load, _) => (Op::Ldr, Size::B32),
            (Self::Store, Size::B8) => (Op::Strb, size),
            (Self::Store, Size::B16) => (Op::Strh, size),
            (Self::Store, _) => (Op::Str, Size::B32),
            (Self::Zext, Size::B8) => (Op::Uxtb, Size::B32),
            (Self::Zext, Size::B16) => (Op::Uxth, Size::B32),
            (Self::Sext, Size::B8) => (Op::Sxtb, Size::B32),
            (Self::Sext, Size::B16) => (Op::Sxth, Size::B32),
            (Self::Zext | Self::Sext, _) => (Op::Mov, Size::B32),
        }
    }

    /// A snippet's step of this instruction at `width`, over `slots`.
    pub(crate) const fn step(self, width: Width<Size>, slots: &'static [Slot<Thumb2>]) -> Step<Thumb2> {
        Step::new(self, width, slots)
    }

    /// Whether this access of `size` between `reg` and memory `disp` bytes past `base` takes one
    /// instruction.
    pub(crate) fn reaches(self, size: Size, reg: Reg, base: Reg, disp: i64) -> bool {
        let (op, size) = self.at(size);
        op.form(size, &[reg.into(), Opnd::Mem(Mem::at(base, disp))]).is_some()
    }
}

impl Emits for Thumb2 {
    type Op = Instr;
    type Size = Size;
    type Operand = Opnd;
    type Cond = Cc;

    const MOV: Instr = Instr::Is(Op::Mov);
    const WHOLE: Size = Size::B32;

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

    /// Every value computes at 32 bits: the bits above a narrow value's own are undefined in a
    /// register.
    fn size(width: Width<Size>, size: Size) -> Size {
        match width {
            Width::Op => size,
            Width::Word => Size::B32,
            Width::Fixed(s) => s,
        }
    }

    fn fits(op: Instr, size: Size, operands: &[Opnd]) -> bool {
        let (op, size) = op.at(size);
        op.form(size, operands).is_some()
    }

    fn implicit(_: Instr) -> &'static [Reg] {
        &[]
    }

    fn emit(code: &mut Asm<Address>, op: Instr, size: Size, operands: &[Opnd]) {
        let (op, size) = op.at(size);
        code.emit(op, size, operands);
    }
}

/// The flags a comparison leaves.
pub trait Flags {
    /// The condition it holds on after `cmp a, b`, or `vcmp a, b` with its flags moved to the
    /// core's; `None` for ordered inequality of floats, which is two conditions. An unordered
    /// `vcmp` sets carry and overflow only, which none of these holds on.
    fn holds(self) -> Option<Cc>;
}

impl Flags for Cond {
    fn holds(self) -> Option<Cc> {
        Some(match self {
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
}

impl vun_native::Native for Thumb2 {
    /// Instructions are 2 or 4 bytes; the return address is in the link register, `r14`.
    const CALL_FRAME: CallFrame = CallFrame {
        word: 4,
        code_align: 2,
        stack_pointer: 13,
        frame_pointer: 7,
        return_address: 14,
        return_pushed: false,
    };

    const CONVENTIONS: &'static [(Convention, Self::Convention)] = vun_asm_thumb::conventions::CONVENTIONS;

    /// 32-bit Arm, its float ABI the convention's.
    ///
    /// # Panics
    /// For a target whose convention this does not follow, which emitting checks first.
    fn machine(target: &TargetSpec) -> Machine {
        let convention = Self::convention(target).expect("checked before emitting");
        Machine::Arm(if convention.registers.float_args.is_empty() { Float::Soft } else { Float::Hard })
    }
}
