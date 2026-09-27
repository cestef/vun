//! arm64: what the framework needs of the architecture (registers, its own moves), the operands
//! and forms snippets are encoded with, AAPCS64, the snippets and the instruction compilers.
//! Homes are reached from the frame pointer, `x29`; `x16` and `x17` are the framework's own, and
//! `x15` holds offsets and addresses too far for one instruction.

pub mod abi;
mod isel;
mod snippets;

use vun_asm::Via;
use vun_asm_arm64::{Arm64, Asm, Cc, Emit, Mem, Op, Operand, Reg, Size};
use vun_codegen::snippet::{Emits, Width};
use vun_codegen::{Anchor, Arch, Place, Recompute};
use vun_dwarf::CallFrame;
use vun_lir::{Address, Cond, Scalar};
use vun_target::{Name, TargetSpec};

use self::abi::Convention;

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
    const CALLEE_SAVED: &'static [Reg] =
        &[Reg::X19, Reg::X20, Reg::X21, Reg::X22, Reg::X23, Reg::X24, Reg::X25, Reg::X26, Reg::X27, Reg::X28];
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
        access(code, Op::Ldr, Size::B64, dst.into(), FP, i64::from(offset), dst);
    }

    fn store(code: &mut Asm<Address>, offset: i32, src: Reg) {
        access(code, Op::Str, Size::B64, src.into(), FP, i64::from(offset), FAR);
    }

    fn recompute(code: &mut Asm<Address>, dst: Reg, what: Recompute) {
        match what {
            Recompute::Const(bits) => {
                constant(code, dst, vun_asm::imm::low64(bits));
            }
            Recompute::Addr(Place { anchor: Anchor::Frame, disp }) => offset(code, dst, FP, disp),
            Recompute::Addr(Place { anchor: Anchor::Sym(target), disp }) => {
                address(code, dst, target);
                offset(code, dst, dst, disp);
            }
        }
    }

    fn jump(code: &mut Asm<Address>, to: vun_asm::Label) {
        code.emit(Op::B, Size::B64, &[to.into()]);
    }
}

/// The 16-bit halves of a 64-bit constant, and their bits.
const HALVES: u32 = 4;
const HALF: u32 = 16;

/// Writes the 64 bits `bits` into `dst`: in one move where one takes them, else a move of the
/// first 16 bits differing from the rest, zeros or (where no 16 bits are zero and most are ones)
/// ones, then a `movk` of each other differing 16 bits, never of zeros, which it cannot shift.
///
/// # Panics
/// Never: a constant no one move takes has 16 bits differing from the rest.
pub fn constant(code: &mut Asm<Address>, dst: Reg, bits: u64) {
    let v = bits.cast_signed();
    if Op::Mov.form(Size::B64, &[Opnd::Reg(dst), Opnd::Imm(v)]).is_some() {
        code.emit(Op::Mov, Size::B64, &[dst.into(), v.into()]);
        return;
    }
    let half = |k: u32| (bits >> (k * HALF)) & 0xffff;
    let ones = (0..HALVES).filter(|&k| half(k) == 0xffff).count();
    let zeros = (0..HALVES).filter(|&k| half(k) == 0).count();
    let fill = if ones > zeros && zeros == 0 { 0xffff } else { 0 };
    let differing: Vec<u32> = (0..HALVES).filter(|&k| half(k) != fill).collect();
    let (&first, rest) = differing.split_first().expect("a constant one move misses differs from its fill");
    let filled = (0..HALVES).filter(|&k| k != first).fold(0u64, |acc, k| acc | fill << (k * HALF));
    let start = filled | half(first) << (first * HALF);
    code.emit(Op::Mov, Size::B64, &[dst.into(), start.cast_signed().into()]);
    for &k in rest {
        code.emit(Op::Movk, Size::B64, &[dst.into(), (half(k) << (k * HALF)).cast_signed().into()]);
    }
}

/// Writes `base + disp` into `dst`, the constant built in `dst` or, when that is the base, in
/// `FAR`.
pub fn offset(code: &mut Asm<Address>, dst: Reg, base: Reg, disp: i64) {
    let takes =
        |op: Op, n: i64| op.form(Size::B64, &[Opnd::Reg(dst), Opnd::Reg(base), Opnd::Imm(n)]).is_some();
    if disp == 0 {
        Arm::copy(code, dst, base);
    } else if takes(Op::Add, disp) {
        code.emit(Op::Add, Size::B64, &[dst.into(), base.into(), disp.into()]);
    } else if takes(Op::Sub, disp.wrapping_neg()) {
        code.emit(Op::Sub, Size::B64, &[dst.into(), base.into(), disp.wrapping_neg().into()]);
    } else {
        let scratch = if dst == base || dst == Reg::Sp { FAR } else { dst };
        constant(code, scratch, disp.cast_unsigned());
        code.emit(Op::Add, Size::B64, &[dst.into(), base.into(), scratch.into()]);
    }
}

/// Writes the address of `target` into `dst`: its page and the offset within it, or for an
/// import, its entry in the global offset table.
pub fn address(code: &mut Asm<Address>, dst: Reg, target: Address) {
    match target {
        Address::Import(_) => {
            code.emit(Op::Adrp, Size::B64, &[dst.into(), Opnd::Page(target, Via::Got)]);
            let entry = Mem::Lo12 { base: dst, target, via: Via::Got };
            code.emit(Op::Ldr, Size::B64, &[dst.into(), entry.into()]);
        }
        Address::Func(_) | Address::Data(_) => {
            code.emit(Op::Adrp, Size::B64, &[dst.into(), Opnd::Page(target, Via::Direct)]);
            code.emit(Op::Add, Size::B64, &[dst.into(), dst.into(), Opnd::Lo12(target)]);
        }
    }
}

/// Whether `op` of `size` between `reg` and memory `disp` bytes past `base` is one instruction.
#[must_use]
pub fn reaches(op: Op, size: Size, reg: Opnd, base: Reg, disp: i64) -> bool {
    op.form(size, &[reg, Opnd::Mem(Mem::at(base, disp))]).is_some()
}

/// `op` of `size` between `reg` and memory `disp` bytes past `base`: at an offset where a form
/// takes it, else at an index built in `scratch`.
pub fn access(code: &mut Asm<Address>, op: Op, size: Size, reg: Opnd, base: Reg, disp: i64, scratch: Reg) {
    let at = Opnd::Mem(Mem::at(base, disp));
    if reaches(op, size, reg, base, disp) {
        code.emit(op, size, &[reg, at]);
    } else {
        constant(code, scratch, disp.cast_unsigned());
        code.emit(op, size, &[reg, Mem::index(base, scratch).into()]);
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

/// The size of an integer, an address or the bits of a float as a general register holds them;
/// `None` for a 128-bit integer and for `f16`, which legalization removes.
#[must_use]
pub fn size_of(scalar: Scalar) -> Option<Size> {
    match scalar {
        Scalar::I8 => Some(Size::B8),
        Scalar::I16 => Some(Size::B16),
        Scalar::I32 | Scalar::F32 => Some(Size::B32),
        Scalar::I64 | Scalar::Ptr | Scalar::F64 => Some(Size::B64),
        Scalar::I128 | Scalar::F16 => None,
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

/// The target ABI following Apple's variant of the call standard.
const DARWIN: Name = Name("darwin");

impl crate::Native for Arm {
    /// Instructions are 4 bytes; the return address is in the link register, `x30`.
    const CALL_FRAME: CallFrame = CallFrame {
        code_align: 4,
        stack_pointer: 31,
        frame_pointer: 29,
        return_address: 30,
        return_pushed: false,
    };

    fn convention(target: &TargetSpec) -> Convention {
        if DARWIN.is(target.abi) { Convention::Darwin } else { Convention::Aapcs64 }
    }
}
