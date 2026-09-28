//! Operations as arm64 instructions (TPDE, section 4). Operands are `In(i)`, the result `Out`,
//! scratch registers `Temp(i)`. Narrow integers compute at 32 bits (`Word`), their upper bits
//! undefined; an operation reading them extends them first, which only 8 and 16 bits need, so
//! those operations have a snippet for narrow operands (`_NARROW`). Floats are held as bits in
//! general registers and computed in `v0` and `v1`, which hold no value between instructions.

use vun_asm_arm64::{Cc, Op, Operand, Reg, Size, V};
use vun_codegen::snippet::Slot::{At, Cond, In, Out, Temp};
use vun_codegen::snippet::{Slot, Snippet, Step, Width};

use super::Arm;

type S = Step<Arm>;

const V0: Slot<Arm> = Slot::Is(Operand::V(V(0)));
const V1: Slot<Arm> = Slot::Is(Operand::V(V(1)));
const ZR: Slot<Arm> = Slot::Is(Operand::Reg(Reg::Zr));
const NE: Slot<Arm> = Slot::Is(Operand::Cond(Cc::Ne));
const MI: Slot<Arm> = Slot::Is(Operand::Cond(Cc::Mi));
const LE: Slot<Arm> = Slot::Is(Operand::Cond(Cc::Le));
/// The byte a boolean is.
const BYTE: Slot<Arm> = Slot::Is(Operand::Imm(0xff));

const OP: Width<Size> = Width::Op;
const WORD: Width<Size> = Width::Word;
const B32: Width<Size> = Width::Fixed(Size::B32);
const B64: Width<Size> = Width::Fixed(Size::B64);

/// `out = a op b`.
macro_rules! three_address {
    ($($name:ident = $op:expr;)*) => {$(
        pub(super) const $name: Snippet<Arm> = &[S::new($op, WORD, &[Out, In(0), In(1)])];
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
        pub(super) const $name: Snippet<Arm> = &[
            S::new($divide, WORD, &[Temp(0), In(0), In(1)]),
            S::new(Op::Msub, WORD, &[Out, Temp(0), In(1), In(0)]),
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
        pub(super) const $name: Snippet<Arm> = &[
            S::new($extend, OP, &[Temp(0), In(0)]),
            S::new($extend, OP, &[Temp(1), In(1)]),
            S::new($op, WORD, &[Out, Temp(0), Temp(1)]),
        ];
    )*};
}

narrow! {
    UDIV_NARROW = Op::Uxt, Op::Udiv;
    SDIV_NARROW = Op::Sxt, Op::Sdiv;
}

macro_rules! narrow_remainder {
    ($($name:ident = $extend:expr, $divide:expr;)*) => {$(
        pub(super) const $name: Snippet<Arm> = &[
            S::new($extend, OP, &[Temp(0), In(0)]),
            S::new($extend, OP, &[Temp(1), In(1)]),
            S::new($divide, WORD, &[Temp(2), Temp(0), Temp(1)]),
            S::new(Op::Msub, WORD, &[Out, Temp(2), Temp(1), Temp(0)]),
        ];
    )*};
}

narrow_remainder! {
    UREM_NARROW = Op::Uxt, Op::Udiv;
    SREM_NARROW = Op::Sxt, Op::Sdiv;
}

/// Right shifts of narrow operands: the shifted one extended, the amount below its width.
pub(super) const SHR_NARROW: Snippet<Arm> =
    &[S::new(Op::Uxt, OP, &[Temp(0), In(0)]), S::new(Op::Lsr, WORD, &[Out, Temp(0), In(1)])];
pub(super) const SAR_NARROW: Snippet<Arm> =
    &[S::new(Op::Sxt, OP, &[Temp(0), In(0)]), S::new(Op::Asr, WORD, &[Out, Temp(0), In(1)])];

/// The flags of `cmp a, b`; `SET` then gives the condition as a byte.
pub(super) const CMP: Snippet<Arm> = &[S::new(Op::Cmp, OP, &[In(0), In(1)])];
pub(super) const SET: Snippet<Arm> =
    &[S::new(Op::Cmp, OP, &[In(0), In(1)]), S::new(Op::Cset, B32, &[Out, Cond])];

/// Comparisons of narrow operands, extended as the comparison reads them.
macro_rules! compare_narrow {
    ($($cmp:ident, $set:ident = $extend:expr;)*) => {$(
        pub(super) const $cmp: Snippet<Arm> = &[
            S::new($extend, OP, &[Temp(0), In(0)]),
            S::new($extend, OP, &[Temp(1), In(1)]),
            S::new(Op::Cmp, B32, &[Temp(0), Temp(1)]),
        ];
        pub(super) const $set: Snippet<Arm> = &[
            S::new($extend, OP, &[Temp(0), In(0)]),
            S::new($extend, OP, &[Temp(1), In(1)]),
            S::new(Op::Cmp, B32, &[Temp(0), Temp(1)]),
            S::new(Op::Cset, B32, &[Out, Cond]),
        ];
    )*};
}

compare_narrow! {
    CMP_UNSIGNED, SET_UNSIGNED = Op::Uxt;
    CMP_SIGNED, SET_SIGNED = Op::Sxt;
}

/// `then` where `cond` is not 0, else `otherwise`: operands `cond`, `then`, `otherwise`.
pub(super) const SELECT: Snippet<Arm> =
    &[S::new(Op::Tst, B32, &[In(0), BYTE]), S::new(Op::Csel, B64, &[Out, In(1), In(2), NE])];

/// Conversions between integers, the operation's width the operand's.
pub(super) const ZEXT: Snippet<Arm> = &[S::new(Op::Uxt, OP, &[Out, In(0)])];
pub(super) const SEXT: Snippet<Arm> = &[S::new(Op::Sxt, OP, &[Out, In(0)])];
/// The same bits: a truncation, an address as an integer, a float's bits.
pub(super) const KEEP: Snippet<Arm> = &[S::new(Op::Mov, B64, &[Out, In(0)])];

/// Memory: a load of the operation's width at the address `In(0)`, zero-extended; a store of
/// `In(0)` at `In(1)`; the same ordered, acquiring and releasing.
pub(super) const LOAD: Snippet<Arm> = &[S::new(Op::Ldr, OP, &[Out, At(0)])];
pub(super) const STORE: Snippet<Arm> = &[S::new(Op::Str, OP, &[In(0), At(1)])];
pub(super) const LOAD_ACQUIRE: Snippet<Arm> = &[S::new(Op::Ldar, OP, &[Out, At(0)])];
pub(super) const STORE_RELEASE: Snippet<Arm> = &[S::new(Op::Stlr, OP, &[In(0), At(1)])];

/// Float arithmetic, the operation's width its precision.
macro_rules! float_binary {
    ($($name:ident = $op:expr;)*) => {$(
        pub(super) const $name: Snippet<Arm> = &[
            S::new(Op::Fmov, OP, &[V0, In(0)]),
            S::new(Op::Fmov, OP, &[V1, In(1)]),
            S::new($op, OP, &[V0, V0, V1]),
            S::new(Op::Fmov, OP, &[Out, V0]),
        ];
    )*};
}

float_binary! {
    FADD = Op::Fadd;
    FSUB = Op::Fsub;
    FMUL = Op::Fmul;
    FDIV = Op::Fdiv;
}

/// The flags of `fcmp a, b`; `FSET` then gives the condition as a byte.
pub(super) const FCMP: Snippet<Arm> = &[
    S::new(Op::Fmov, OP, &[V0, In(0)]),
    S::new(Op::Fmov, OP, &[V1, In(1)]),
    S::new(Op::Fcmp, OP, &[V0, V1]),
];
pub(super) const FSET: Snippet<Arm> = &[
    S::new(Op::Fmov, OP, &[V0, In(0)]),
    S::new(Op::Fmov, OP, &[V1, In(1)]),
    S::new(Op::Fcmp, OP, &[V0, V1]),
    S::new(Op::Cset, B32, &[Out, Cond]),
];
/// Ordered inequality: less, else greater (1 unless less or equal or unordered).
pub(super) const FNE: Snippet<Arm> = &[
    S::new(Op::Fmov, OP, &[V0, In(0)]),
    S::new(Op::Fmov, OP, &[V1, In(1)]),
    S::new(Op::Fcmp, OP, &[V0, V1]),
    S::new(Op::Cset, B32, &[Out, MI]),
    S::new(Op::Csinc, B32, &[Out, Out, ZR, LE]),
];

pub(super) const WIDEN: Snippet<Arm> = &[
    S::new(Op::Fmov, B32, &[V0, In(0)]),
    S::new(Op::Fcvt, B64, &[V0, V0]),
    S::new(Op::Fmov, B64, &[Out, V0]),
];
pub(super) const NARROW: Snippet<Arm> = &[
    S::new(Op::Fmov, B64, &[V0, In(0)]),
    S::new(Op::Fcvt, B32, &[V0, V0]),
    S::new(Op::Fmov, B32, &[Out, V0]),
];

/// An integer of 32 or 64 bits, the operation's width, to a float.
macro_rules! to_float {
    ($($name:ident = $convert:expr, $precision:expr;)*) => {$(
        pub(super) const $name: Snippet<Arm> =
            &[S::new($convert, OP, &[V0, In(0)]), S::new(Op::Fmov, $precision, &[Out, V0])];
    )*};
}

to_float! {
    SIGNED_TO_SINGLE = Op::ScvtfS, B32;
    SIGNED_TO_DOUBLE = Op::ScvtfD, B64;
    UNSIGNED_TO_SINGLE = Op::UcvtfS, B32;
    UNSIGNED_TO_DOUBLE = Op::UcvtfD, B64;
}

/// A narrow integer, the operation's width, to a float: extended to 64 bits as it reads.
macro_rules! narrow_to_float {
    ($($name:ident = $extend:expr, $convert:expr, $precision:expr;)*) => {$(
        pub(super) const $name: Snippet<Arm> = &[
            S::new($extend, OP, &[Temp(0), In(0)]),
            S::new($convert, B64, &[V0, Temp(0)]),
            S::new(Op::Fmov, $precision, &[Out, V0]),
        ];
    )*};
}

narrow_to_float! {
    NARROW_SIGNED_TO_SINGLE = Op::Sxt, Op::ScvtfS, B32;
    NARROW_SIGNED_TO_DOUBLE = Op::Sxt, Op::ScvtfD, B64;
    NARROW_UNSIGNED_TO_SINGLE = Op::Uxt, Op::UcvtfS, B32;
    NARROW_UNSIGNED_TO_DOUBLE = Op::Uxt, Op::UcvtfD, B64;
}

/// A float, the operation's width its precision, rounded toward zero to a 64-bit integer, whose
/// low bits are every narrower result.
macro_rules! to_int {
    ($($name:ident = $convert:expr;)*) => {$(
        pub(super) const $name: Snippet<Arm> =
            &[S::new(Op::Fmov, OP, &[V0, In(0)]), S::new($convert, B64, &[Out, V0])];
    )*};
}

to_int! {
    SINGLE_TO_SIGNED = Op::FcvtzsS;
    DOUBLE_TO_SIGNED = Op::FcvtzsD;
    SINGLE_TO_UNSIGNED = Op::FcvtzuS;
    DOUBLE_TO_UNSIGNED = Op::FcvtzuD;
}
