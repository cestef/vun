//! Integer, memory and control instructions (Intel SDM volume 2).

use super::form::Kind::{Cl, Cond, I8, I64, Iz, M, R, RM, RMn, Rel, Rn, U32};
use super::form::{Form, Layout, Opcode, Prefix, Sizes, W};
use crate::reg::{Reg, Size};

const DST_SRC: Layout = Layout::RegRm { reg: 1, rm: 0 };
const SRC_DST: Layout = Layout::RegRm { reg: 0, rm: 1 };
const NONE: &[super::form::Kind] = &[];

/// `add`, `or`, `and`, `sub`, `xor` and `cmp`, by their opcode extension.
pub(super) const fn alu(digit: u8) -> [Form; 4] {
    let base = digit << 3;
    [
        Form::new(&[RM, R], Sizes::ALL, Opcode::one(base | 1), DST_SRC).byte(base),
        Form::new(&[R, RM], Sizes::ALL, Opcode::one(base | 3), SRC_DST).byte(base | 2),
        Form::new(&[RM, I8], Sizes::WORDS, Opcode::one(0x83), Layout::Digit { digit, rm: 0 }),
        Form::new(&[RM, Iz], Sizes::ALL, Opcode::one(0x81), Layout::Digit { digit, rm: 0 }).byte(0x80),
    ]
}

/// The one-operand group `F7`: `not`, `neg`, and `mul`, `div`, `idiv` over `rdx:rax`.
pub(super) const fn unary(digit: u8, implicit: &'static [Reg]) -> [Form; 1] {
    [Form::new(&[RM], Sizes::ALL, Opcode::one(0xf7), Layout::Digit { digit, rm: 0 })
        .byte(0xf6)
        .implicit(implicit)]
}

/// `rdx:rax`, which multiplication and division read and write.
pub(super) const WIDE_ACCUMULATOR: &[Reg] = &[Reg::Rax, Reg::Rdx];

/// Shifts by `cl` or by an immediate.
pub(super) const fn shift(digit: u8) -> [Form; 2] {
    [
        Form::new(&[RM, Cl], Sizes::ALL, Opcode::one(0xd3), Layout::Digit { digit, rm: 0 }).byte(0xd2),
        Form::new(&[RM, I8], Sizes::ALL, Opcode::one(0xc1), Layout::Digit { digit, rm: 0 }).byte(0xc0),
    ]
}

pub(super) const MOV: [Form; 5] = [
    Form::new(&[RM, R], Sizes::ALL, Opcode::one(0x89), DST_SRC).byte(0x88),
    Form::new(&[R, RM], Sizes::ALL, Opcode::one(0x8b), SRC_DST).byte(0x8a),
    // Writing a 32-bit register clears the upper half.
    Form::new(&[Rn(Size::B32), U32], Sizes::WIDE, Opcode::one(0xb8), Layout::PlusReg(0)).w(W::Never),
    Form::new(&[RM, Iz], Sizes::WIDE, Opcode::one(0xc7), Layout::Digit { digit: 0, rm: 0 }),
    Form::new(&[R, I64], Sizes::of(&[Size::B64]), Opcode::one(0xb8), Layout::PlusReg(0)),
];

/// By the width of the operand extended.
pub(super) const MOVZX: [Form; 4] = [
    Form::new(&[Rn(Size::B32), RMn(Size::B8)], Sizes::of(&[Size::B8]), Opcode::two(0xb6), SRC_DST)
        .w(W::Never),
    Form::new(&[Rn(Size::B32), RMn(Size::B16)], Sizes::of(&[Size::B16]), Opcode::two(0xb7), SRC_DST)
        .w(W::Never),
    Form::new(&[Rn(Size::B32), RMn(Size::B32)], Sizes::of(&[Size::B32]), Opcode::one(0x8b), SRC_DST)
        .w(W::Never)
        .named("mov"),
    Form::new(&[Rn(Size::B64), RMn(Size::B64)], Sizes::of(&[Size::B64]), Opcode::one(0x8b), SRC_DST)
        .w(W::Always)
        .named("mov"),
];

/// By the width of the operand extended.
pub(super) const MOVSX: [Form; 4] = [
    Form::new(&[Rn(Size::B64), RMn(Size::B8)], Sizes::of(&[Size::B8]), Opcode::two(0xbe), SRC_DST)
        .w(W::Always),
    Form::new(&[Rn(Size::B64), RMn(Size::B16)], Sizes::of(&[Size::B16]), Opcode::two(0xbf), SRC_DST)
        .w(W::Always),
    Form::new(&[Rn(Size::B64), RMn(Size::B32)], Sizes::of(&[Size::B32]), Opcode::one(0x63), SRC_DST)
        .w(W::Always)
        .named("movsxd"),
    Form::new(&[Rn(Size::B64), RMn(Size::B64)], Sizes::of(&[Size::B64]), Opcode::one(0x8b), SRC_DST)
        .w(W::Always)
        .named("mov"),
];

pub(super) const LEA: [Form; 1] = [Form::new(&[R, M], Sizes::WIDE, Opcode::one(0x8d), SRC_DST)];

pub(super) const TEST: [Form; 2] = [
    Form::new(&[RM, R], Sizes::ALL, Opcode::one(0x85), DST_SRC).byte(0x84),
    Form::new(&[RM, Iz], Sizes::ALL, Opcode::one(0xf7), Layout::Digit { digit: 0, rm: 0 }).byte(0xf6),
];

pub(super) const IMUL: [Form; 3] = [
    Form::new(&[R, RM], Sizes::WORDS, Opcode::two(0xaf), SRC_DST),
    Form::new(&[R, RM, I8], Sizes::WORDS, Opcode::one(0x6b), SRC_DST),
    Form::new(&[R, RM, Iz], Sizes::WORDS, Opcode::one(0x69), SRC_DST),
];

pub(super) const CQO: [Form; 2] = [
    Form::new(NONE, Sizes::of(&[Size::B32]), Opcode::one(0x99), Layout::Bare)
        .named("cdq")
        .implicit(WIDE_ACCUMULATOR),
    Form::new(NONE, Sizes::of(&[Size::B64]), Opcode::one(0x99), Layout::Bare).implicit(WIDE_ACCUMULATOR),
];

pub(super) const SET: [Form; 1] =
    [Form::new(&[Cond, RMn(Size::B8)], Sizes::ALL, Opcode::two(0x90), Layout::Digit { digit: 0, rm: 1 })
        .w(W::Never)];
pub(super) const CMOV: [Form; 1] =
    [Form::new(&[Cond, R, RM], Sizes::WORDS, Opcode::two(0x40), Layout::RegRm { reg: 1, rm: 2 })];

pub(super) const JCC: [Form; 1] =
    [Form::new(&[Cond, Rel], Sizes::ALL, Opcode::two(0x80), Layout::Bare).w(W::Never)];
pub(super) const JMP: [Form; 1] =
    [Form::new(&[Rel], Sizes::ALL, Opcode::one(0xe9), Layout::Bare).w(W::Never)];
/// A call's operand is 64 bits without REX.W.
pub(super) const CALL: [Form; 2] = [
    Form::new(&[Rel], Sizes::ALL, Opcode::one(0xe8), Layout::Bare).w(W::Never),
    Form::new(&[RMn(Size::B64)], Sizes::ALL, Opcode::one(0xff), Layout::Digit { digit: 2, rm: 0 })
        .w(W::Never),
];
pub(super) const RET: [Form; 1] = [Form::new(NONE, Sizes::ALL, Opcode::one(0xc3), Layout::Bare).w(W::Never)];
pub(super) const LEAVE: [Form; 1] =
    [Form::new(NONE, Sizes::ALL, Opcode::one(0xc9), Layout::Bare).w(W::Never)];
pub(super) const UD2: [Form; 1] = [Form::new(NONE, Sizes::ALL, Opcode::two(0x0b), Layout::Bare).w(W::Never)];
pub(super) const PUSH: [Form; 1] =
    [Form::new(&[Rn(Size::B64)], Sizes::ALL, Opcode::one(0x50), Layout::PlusReg(0)).w(W::Never)];
pub(super) const POP: [Form; 1] =
    [Form::new(&[Rn(Size::B64)], Sizes::ALL, Opcode::one(0x58), Layout::PlusReg(0)).w(W::Never)];

pub(super) const XCHG: [Form; 1] = [Form::new(&[RM, R], Sizes::ALL, Opcode::one(0x87), DST_SRC).byte(0x86)];
pub(super) const XADD: [Form; 1] =
    [Form::new(&[M, R], Sizes::ALL, Opcode::two(0xc1), DST_SRC).byte(0xc0).prefix(Prefix::Lock)];
pub(super) const CMPXCHG: [Form; 1] = [Form::new(&[M, R], Sizes::ALL, Opcode::two(0xb1), DST_SRC)
    .byte(0xb0)
    .prefix(Prefix::Lock)
    .implicit(&[Reg::Rax])];
pub(super) const MOVSB: [Form; 1] = [Form::new(NONE, Sizes::ALL, Opcode::one(0xa4), Layout::Bare)
    .w(W::Never)
    .prefix(Prefix::F3)
    .implicit(&[Reg::Rsi, Reg::Rdi, Reg::Rcx])];
