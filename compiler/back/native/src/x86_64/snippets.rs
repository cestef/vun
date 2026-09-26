//! Operations as x86-64 instructions (TPDE, section 4). Operands are `In(i)`, the result `Out`,
//! scratch registers `Temp(i)`; `Fixed` registers are freed before a snippet starts. Narrow
//! integers compute at 32 bits (`Word`), their upper bits undefined; an operation reading them
//! extends them first. Floats are held as bits in general registers and computed in `xmm0` and
//! `xmm1`, which hold no value between instructions.

use vun_asm_x86_64::{Cc, Op, Operand, Reg, Size, Xmm};
use vun_codegen::snippet::Slot::{At, Cond, In, Out, Temp};
use vun_codegen::snippet::{Slot, Snippet, Step, Width};

use super::X86;

type S = Step<X86>;

const RAX: Slot<X86> = Slot::Fixed(Reg::Rax);
const RDX: Slot<X86> = Slot::Fixed(Reg::Rdx);
const X0: Slot<X86> = Slot::Is(Operand::Xmm(Xmm(0)));
const X1: Slot<X86> = Slot::Is(Operand::Xmm(Xmm(1)));
const NE: Slot<X86> = Slot::Is(Operand::Cond(Cc::Ne));
const NP: Slot<X86> = Slot::Is(Operand::Cond(Cc::Np));

const OP: Width<Size> = Width::Op;
const WORD: Width<Size> = Width::Word;
const B8: Width<Size> = Width::Fixed(Size::B8);
const B32: Width<Size> = Width::Fixed(Size::B32);
const B64: Width<Size> = Width::Fixed(Size::B64);

/// `out = a op b`, over the first operand's copy.
macro_rules! two_address {
    ($($name:ident = $op:expr;)*) => {$(
        pub(super) const $name: Snippet<X86> = &[S::new(Op::Mov, WORD, &[Out, In(0)]), S::new($op, WORD, &[Out, In(1)])];
    )*};
}

two_address! {
    ADD = Op::Add;
    SUB = Op::Sub;
    AND = Op::And;
    OR = Op::Or;
    XOR = Op::Xor;
    MUL = Op::Imul;
}

/// Division through `rax` and `rdx`, the operands extended as the division reads them; `$out`
/// holds the quotient or the remainder.
macro_rules! divide {
    ($($name:ident = $extend:expr, $sign:expr, $divide:expr, $out:expr;)*) => {$(
        pub(super) const $name: Snippet<X86> = &[
            S::new($extend, OP, &[RAX, In(0)]),
            S::new($extend, OP, &[Temp(0), In(1)]),
            $sign,
            S::new($divide, WORD, &[Temp(0)]),
            S::new(Op::Mov, B64, &[Out, $out]),
        ];
    )*};
}

const CLEAR_RDX: S = S::new(Op::Xor, B32, &[RDX, RDX]);
const SIGN_RDX: S = S::new(Op::Cqo, WORD, &[]);

divide! {
    UDIV = Op::Movzx, CLEAR_RDX, Op::Div, RAX;
    UREM = Op::Movzx, CLEAR_RDX, Op::Div, RDX;
    SDIV = Op::Movsx, SIGN_RDX, Op::Idiv, RAX;
    SREM = Op::Movsx, SIGN_RDX, Op::Idiv, RDX;
}

/// Shifts, the shifted operand extended as the shift reads it, by a constant or by `cl`.
macro_rules! shift {
    ($($name:ident = $extend:expr, $shift:expr;)*) => {$(
        pub(super) const $name: Snippet<X86> =
            &[S::new($extend, OP, &[Out, In(0)]), S::new($shift, WORD, &[Out, COUNT])];
    )*};
}

const COUNT: Slot<X86> = Slot::Through(Reg::Rcx, 1);

shift! {
    SHL = Op::Mov, Op::Shl;
    SHR = Op::Movzx, Op::Shr;
    SAR = Op::Movsx, Op::Sar;
}

/// The flags of `cmp a, b`; `SET` then gives the condition as a byte.
pub(super) const CMP: Snippet<X86> = &[S::new(Op::Cmp, OP, &[In(0), In(1)])];
pub(super) const SET: Snippet<X86> =
    &[S::new(Op::Cmp, OP, &[In(0), In(1)]), S::new(Op::Set, B8, &[Cond, Out])];

/// `then` where `cond` is not 0, else `otherwise`: operands `cond`, `then`, `otherwise`.
pub(super) const SELECT: Snippet<X86> = &[
    S::new(Op::Mov, B64, &[Out, In(2)]),
    S::new(Op::Test, B8, &[In(0), In(0)]),
    S::new(Op::Cmov, B64, &[NE, Out, In(1)]),
];

/// Conversions between integers, the operation's width the operand's.
pub(super) const ZEXT: Snippet<X86> = &[S::new(Op::Movzx, OP, &[Out, In(0)])];
pub(super) const SEXT: Snippet<X86> = &[S::new(Op::Movsx, OP, &[Out, In(0)])];
/// The same bits: a truncation, an address as an integer, a float's bits.
pub(super) const KEEP: Snippet<X86> = &[S::new(Op::Mov, B64, &[Out, In(0)])];

/// Memory: a load of the operation's width at the address `In(0)`; a store of `In(0)` at `In(1)`.
pub(super) const LOAD: Snippet<X86> = &[S::new(Op::Movzx, OP, &[Out, At(0)])];
pub(super) const STORE: Snippet<X86> = &[S::new(Op::Mov, OP, &[At(1), In(0)])];
/// A sequentially consistent store exchanges.
pub(super) const STORE_SEQ_CST: Snippet<X86> =
    &[S::new(Op::Mov, B64, &[Temp(0), In(0)]), S::new(Op::Xchg, OP, &[At(1), Temp(0)])];
/// Atomic operations on memory at `In(0)` with `In(1)`, giving what it held.
pub(super) const SWAP: Snippet<X86> =
    &[S::new(Op::Mov, B64, &[Out, In(1)]), S::new(Op::Xchg, OP, &[At(0), Out])];
pub(super) const FETCH_ADD: Snippet<X86> =
    &[S::new(Op::Mov, B64, &[Out, In(1)]), S::new(Op::LockXadd, OP, &[At(0), Out])];
pub(super) const FETCH_SUB: Snippet<X86> = &[
    S::new(Op::Mov, B64, &[Out, In(1)]),
    S::new(Op::Neg, WORD, &[Out]),
    S::new(Op::LockXadd, OP, &[At(0), Out]),
];
/// Writes `In(2)` at `In(0)` where it holds `In(1)`; gives what it held.
pub(super) const COMPARE_EXCHANGE: Snippet<X86> = &[
    S::new(Op::Mov, B64, &[RAX, In(1)]),
    S::new(Op::LockCmpxchg, OP, &[At(0), In(2)]),
    S::new(Op::Mov, B64, &[Out, RAX]),
];

/// Float arithmetic, the operation's width its precision.
macro_rules! float_binary {
    ($($name:ident = $op:expr;)*) => {$(
        pub(super) const $name: Snippet<X86> = &[
            S::new(Op::Movd, OP, &[X0, In(0)]),
            S::new(Op::Movd, OP, &[X1, In(1)]),
            S::new($op, OP, &[X0, X1]),
            S::new(Op::Movd, OP, &[Out, X0]),
        ];
    )*};
}

float_binary! {
    FADD = Op::Adds;
    FSUB = Op::Subs;
    FMUL = Op::Muls;
    FDIV = Op::Divs;
}

/// An ordered comparison holding on the flags `Cond` gives after `ucomis In(0), In(1)`: false
/// where unordered. `a < b` is compared as `b > a`, which the caller orders.
pub(super) const FCOMPARE: Snippet<X86> = &[
    S::new(Op::Movd, OP, &[X0, In(0)]),
    S::new(Op::Movd, OP, &[X1, In(1)]),
    S::new(Op::Ucomis, OP, &[X0, X1]),
    S::new(Op::Set, B8, &[Cond, Out]),
];
/// (In)equality, which also needs the parity clear: ordered.
pub(super) const FEQUAL: Snippet<X86> = &[
    S::new(Op::Movd, OP, &[X0, In(0)]),
    S::new(Op::Movd, OP, &[X1, In(1)]),
    S::new(Op::Ucomis, OP, &[X0, X1]),
    S::new(Op::Set, B8, &[Cond, Out]),
    S::new(Op::Set, B8, &[NP, Temp(0)]),
    S::new(Op::And, B8, &[Out, Temp(0)]),
];

pub(super) const WIDEN: Snippet<X86> = &[
    S::new(Op::Movd, B32, &[X0, In(0)]),
    S::new(Op::Cvtss2sd, B64, &[X0, X0]),
    S::new(Op::Movd, B64, &[Out, X0]),
];
pub(super) const NARROW: Snippet<X86> = &[
    S::new(Op::Movd, B64, &[X0, In(0)]),
    S::new(Op::Cvtsd2ss, B32, &[X0, X0]),
    S::new(Op::Movd, B32, &[Out, X0]),
];

/// An integer of the operation's width to a float: extended to 64 bits as it reads, converted
/// as a signed `i64`, which holds every value of a narrower integer.
macro_rules! to_float {
    ($($name:ident = $extend:expr, $convert:expr, $precision:expr;)*) => {$(
        pub(super) const $name: Snippet<X86> = &[
            S::new($extend, OP, &[Temp(0), In(0)]),
            S::new($convert, B64, &[X0, Temp(0)]),
            S::new(Op::Movd, $precision, &[Out, X0]),
        ];
    )*};
}

to_float! {
    SIGNED_TO_SINGLE = Op::Movsx, Op::Cvtsi2ss, B32;
    SIGNED_TO_DOUBLE = Op::Movsx, Op::Cvtsi2sd, B64;
    UNSIGNED_TO_SINGLE = Op::Movzx, Op::Cvtsi2ss, B32;
    UNSIGNED_TO_DOUBLE = Op::Movzx, Op::Cvtsi2sd, B64;
}

/// A float truncated to an `i64`, whose low bits are every narrower result.
pub(super) const SINGLE_TO_INT: Snippet<X86> =
    &[S::new(Op::Movd, B32, &[X0, In(0)]), S::new(Op::Cvttss2si, B64, &[Out, X0])];
pub(super) const DOUBLE_TO_INT: Snippet<X86> =
    &[S::new(Op::Movd, B64, &[X0, In(0)]), S::new(Op::Cvttsd2si, B64, &[Out, X0])];
