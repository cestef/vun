//! Operations as Thumb instructions (TPDE, section 4). Operands are `In(i)`, the result `Out`,
//! scratch registers `Temp(i)`. Every value computes at 32 bits (`Word`), a narrow one's upper
//! bits undefined; an operation reading them extends them first, so those operations have a
//! snippet for narrow operands (`_NARROW`). A condition becomes a byte through an `it` block,
//! whose moves the encoder emits as they are. Floats are single precision, held as bits in
//! general registers and computed in `s0` and `s1`, which hold no value between instructions.

use vun_asm_thumb::{Barrier, Cc, Op, Operand, Size, Status, V};
use vun_codegen::snippet::Slot::{At, Cond, In, Out, Temp};
use vun_codegen::snippet::{Slot, Snippet, Width};

use super::{Instr, Thumb2};

const S0: Slot<Thumb2> = Slot::Is(Operand::V(V::V0));
const S1: Slot<Thumb2> = Slot::Is(Operand::V(V::V1));
const NE: Slot<Thumb2> = Slot::Is(Operand::Cond(Cc::Ne));
const MI: Slot<Thumb2> = Slot::Is(Operand::Cond(Cc::Mi));
const GT: Slot<Thumb2> = Slot::Is(Operand::Cond(Cc::Gt));
const FLAGS: Slot<Thumb2> = Slot::Is(Operand::Status(Status::ApsrNzcv));
const FPSCR: Slot<Thumb2> = Slot::Is(Operand::Status(Status::Fpscr));
const SY: Slot<Thumb2> = Slot::Is(Operand::Barrier(Barrier::Sy));
/// The byte a boolean is, and its two values.
const BYTE: Slot<Thumb2> = Slot::Is(Operand::Imm(0xff));
const FALSE: Slot<Thumb2> = Slot::Is(Operand::Imm(0));
const TRUE: Slot<Thumb2> = Slot::Is(Operand::Imm(1));

const OP: Width<Size> = Width::Op;
const WORD: Width<Size> = Width::Word;
const B32: Width<Size> = Width::Fixed(Size::B32);

/// `out = a op b`.
macro_rules! three_address {
    ($($name:ident = $op:expr;)*) => {$(
        pub(super) const $name: Snippet<Thumb2> = &[Instr::Is($op).step(WORD, &[Out, In(0), In(1)])];
    )*};
}

three_address! {
    ADD = Op::Add;
    SUB = Op::Sub;
    AND = Op::And;
    OR = Op::Orr;
    XOR = Op::Eor;
    MUL = Op::Mul;
    SHL = Op::Lsl;
    SHR = Op::Lsr;
    SAR = Op::Asr;
    UDIV = Op::Udiv;
    SDIV = Op::Sdiv;
}

/// A remainder: the dividend less the quotient times the divisor.
macro_rules! remainder {
    ($($name:ident = $divide:expr;)*) => {$(
        pub(super) const $name: Snippet<Thumb2> = &[
            Instr::Is($divide).step(WORD, &[Temp(0), In(0), In(1)]),
            Instr::Is(Op::Mls).step(WORD, &[Out, Temp(0), In(1), In(0)]),
        ];
    )*};
}

remainder! {
    UREM = Op::Udiv;
    SREM = Op::Sdiv;
}

/// Operations on narrow operands, each extended as the operation reads it.
macro_rules! narrow {
    ($($name:ident = $extend:expr, $op:expr;)*) => {$(
        pub(super) const $name: Snippet<Thumb2> = &[
            $extend.step(OP, &[Temp(0), In(0)]),
            $extend.step(OP, &[Temp(1), In(1)]),
            Instr::Is($op).step(WORD, &[Out, Temp(0), Temp(1)]),
        ];
    )*};
}

narrow! {
    UDIV_NARROW = Instr::Zext, Op::Udiv;
    SDIV_NARROW = Instr::Sext, Op::Sdiv;
}

macro_rules! narrow_remainder {
    ($($name:ident = $extend:expr, $divide:expr;)*) => {$(
        pub(super) const $name: Snippet<Thumb2> = &[
            $extend.step(OP, &[Temp(0), In(0)]),
            $extend.step(OP, &[Temp(1), In(1)]),
            Instr::Is($divide).step(WORD, &[Temp(2), Temp(0), Temp(1)]),
            Instr::Is(Op::Mls).step(WORD, &[Out, Temp(2), Temp(1), Temp(0)]),
        ];
    )*};
}

narrow_remainder! {
    UREM_NARROW = Instr::Zext, Op::Udiv;
    SREM_NARROW = Instr::Sext, Op::Sdiv;
}

/// Right shifts of narrow operands: the shifted one extended, the amount below its width.
pub(super) const SHR_NARROW: Snippet<Thumb2> =
    &[Instr::Zext.step(OP, &[Temp(0), In(0)]), Instr::Is(Op::Lsr).step(WORD, &[Out, Temp(0), In(1)])];
pub(super) const SAR_NARROW: Snippet<Thumb2> =
    &[Instr::Sext.step(OP, &[Temp(0), In(0)]), Instr::Is(Op::Asr).step(WORD, &[Out, Temp(0), In(1)])];

/// The flags of `cmp a, b`; `SET` then gives the condition as a byte: 0, then 1 where it holds.
pub(super) const CMP: Snippet<Thumb2> = &[Instr::Is(Op::Cmp).step(WORD, &[In(0), In(1)])];
pub(super) const SET: Snippet<Thumb2> = &[
    Instr::Is(Op::Cmp).step(WORD, &[In(0), In(1)]),
    Instr::Is(Op::Mov).step(B32, &[Out, FALSE]),
    Instr::Is(Op::It).step(B32, &[Cond]),
    Instr::Then(Op::Mov).step(B32, &[Out, TRUE]),
];

/// Comparisons of narrow operands, extended as the comparison reads them.
macro_rules! compare_narrow {
    ($($cmp:ident, $set:ident = $extend:expr;)*) => {$(
        pub(super) const $cmp: Snippet<Thumb2> = &[
            $extend.step(OP, &[Temp(0), In(0)]),
            $extend.step(OP, &[Temp(1), In(1)]),
            Instr::Is(Op::Cmp).step(B32, &[Temp(0), Temp(1)]),
        ];
        pub(super) const $set: Snippet<Thumb2> = &[
            $extend.step(OP, &[Temp(0), In(0)]),
            $extend.step(OP, &[Temp(1), In(1)]),
            Instr::Is(Op::Cmp).step(B32, &[Temp(0), Temp(1)]),
            Instr::Is(Op::Mov).step(B32, &[Out, FALSE]),
            Instr::Is(Op::It).step(B32, &[Cond]),
            Instr::Then(Op::Mov).step(B32, &[Out, TRUE]),
        ];
    )*};
}

compare_narrow! {
    CMP_UNSIGNED, SET_UNSIGNED = Instr::Zext;
    CMP_SIGNED, SET_SIGNED = Instr::Sext;
}

/// `then` where `cond` is not 0, else `otherwise`: operands `cond`, `then`, `otherwise`. `then`
/// is brought into a scratch register before the `it` block, which a register alone may be read in.
pub(super) const SELECT: Snippet<Thumb2> = &[
    Instr::Is(Op::Mov).step(B32, &[Temp(0), In(1)]),
    Instr::Is(Op::Tst).step(B32, &[In(0), BYTE]),
    Instr::Is(Op::Mov).step(B32, &[Out, In(2)]),
    Instr::Is(Op::It).step(B32, &[NE]),
    Instr::Then(Op::Mov).step(B32, &[Out, Temp(0)]),
];

/// Conversions between integers, the operation's width the operand's.
pub(super) const ZEXT: Snippet<Thumb2> = &[Instr::Zext.step(OP, &[Out, In(0)])];
pub(super) const SEXT: Snippet<Thumb2> = &[Instr::Sext.step(OP, &[Out, In(0)])];
/// The same bits: a truncation, an address as an integer, a float's bits.
pub(super) const KEEP: Snippet<Thumb2> = &[Instr::Is(Op::Mov).step(B32, &[Out, In(0)])];

/// Memory: a load of the operation's width at the address `In(0)`, zero-extended; a store of
/// `In(0)` at `In(1)`; the same ordered, a barrier after a load and on both sides of a store,
/// which together are sequentially consistent.
pub(super) const LOAD: Snippet<Thumb2> = &[Instr::Load.step(OP, &[Out, At(0)])];
pub(super) const STORE: Snippet<Thumb2> = &[Instr::Store.step(OP, &[In(0), At(1)])];
pub(super) const LOAD_ACQUIRE: Snippet<Thumb2> =
    &[Instr::Load.step(OP, &[Out, At(0)]), Instr::Is(Op::Dmb).step(B32, &[SY])];
pub(super) const STORE_RELEASE: Snippet<Thumb2> = &[
    Instr::Is(Op::Dmb).step(B32, &[SY]),
    Instr::Store.step(OP, &[In(0), At(1)]),
    Instr::Is(Op::Dmb).step(B32, &[SY]),
];

/// Float arithmetic.
macro_rules! float_binary {
    ($($name:ident = $op:expr;)*) => {$(
        pub(super) const $name: Snippet<Thumb2> = &[
            Instr::Is(Op::Vmov).step(B32, &[S0, In(0)]),
            Instr::Is(Op::Vmov).step(B32, &[S1, In(1)]),
            Instr::Is($op).step(B32, &[S0, S0, S1]),
            Instr::Is(Op::Vmov).step(B32, &[Out, S0]),
        ];
    )*};
}

float_binary! {
    FADD = Op::Vadd;
    FSUB = Op::Vsub;
    FMUL = Op::Vmul;
    FDIV = Op::Vdiv;
}

/// The flags of `vcmp a, b`, moved to the core's; `FSET` then gives the condition as a byte.
pub(super) const FCMP: Snippet<Thumb2> = &[
    Instr::Is(Op::Vmov).step(B32, &[S0, In(0)]),
    Instr::Is(Op::Vmov).step(B32, &[S1, In(1)]),
    Instr::Is(Op::Vcmp).step(B32, &[S0, S1]),
    Instr::Is(Op::Vmrs).step(B32, &[FLAGS, FPSCR]),
];
pub(super) const FSET: Snippet<Thumb2> = &[
    Instr::Is(Op::Vmov).step(B32, &[S0, In(0)]),
    Instr::Is(Op::Vmov).step(B32, &[S1, In(1)]),
    Instr::Is(Op::Vcmp).step(B32, &[S0, S1]),
    Instr::Is(Op::Vmrs).step(B32, &[FLAGS, FPSCR]),
    Instr::Is(Op::Mov).step(B32, &[Out, FALSE]),
    Instr::Is(Op::It).step(B32, &[Cond]),
    Instr::Then(Op::Mov).step(B32, &[Out, TRUE]),
];
/// The flags of a `vcmp` made already, moved to the core's and given as a byte: a double's, whose
/// operands no snippet reads.
pub(super) const FLAGS_SET: Snippet<Thumb2> = &[
    Instr::Is(Op::Vmrs).step(B32, &[FLAGS, FPSCR]),
    Instr::Is(Op::Mov).step(B32, &[Out, FALSE]),
    Instr::Is(Op::It).step(B32, &[Cond]),
    Instr::Then(Op::Mov).step(B32, &[Out, TRUE]),
];
/// Ordered inequality: less, or greater.
pub(super) const FNE: Snippet<Thumb2> = &[
    Instr::Is(Op::Vmov).step(B32, &[S0, In(0)]),
    Instr::Is(Op::Vmov).step(B32, &[S1, In(1)]),
    Instr::Is(Op::Vcmp).step(B32, &[S0, S1]),
    Instr::Is(Op::Vmrs).step(B32, &[FLAGS, FPSCR]),
    Instr::Is(Op::Mov).step(B32, &[Out, FALSE]),
    Instr::Is(Op::It).step(B32, &[MI]),
    Instr::Then(Op::Mov).step(B32, &[Out, TRUE]),
    Instr::Is(Op::It).step(B32, &[GT]),
    Instr::Then(Op::Mov).step(B32, &[Out, TRUE]),
];

/// A 32-bit integer to a float, and a float rounded toward zero to one, whose low bits are every
/// narrower result.
macro_rules! convert {
    ($($name:ident = $convert:expr;)*) => {$(
        pub(super) const $name: Snippet<Thumb2> = &[
            Instr::Is(Op::Vmov).step(B32, &[S0, In(0)]),
            Instr::Is($convert).step(B32, &[S0, S0]),
            Instr::Is(Op::Vmov).step(B32, &[Out, S0]),
        ];
    )*};
}

convert! {
    SIGNED_TO_SINGLE = Op::VcvtFromSigned;
    UNSIGNED_TO_SINGLE = Op::VcvtFromUnsigned;
    SINGLE_TO_SIGNED = Op::VcvtSigned;
    SINGLE_TO_UNSIGNED = Op::VcvtUnsigned;
}

/// A narrow integer, the operation's width, to a float: extended as it is read.
macro_rules! narrow_to_float {
    ($($name:ident = $extend:expr, $convert:expr;)*) => {$(
        pub(super) const $name: Snippet<Thumb2> = &[
            $extend.step(OP, &[Temp(0), In(0)]),
            Instr::Is(Op::Vmov).step(B32, &[S0, Temp(0)]),
            Instr::Is($convert).step(B32, &[S0, S0]),
            Instr::Is(Op::Vmov).step(B32, &[Out, S0]),
        ];
    )*};
}

narrow_to_float! {
    NARROW_SIGNED_TO_SINGLE = Instr::Sext, Op::VcvtFromSigned;
    NARROW_UNSIGNED_TO_SINGLE = Instr::Zext, Op::VcvtFromUnsigned;
}
