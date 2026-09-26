//! SSE instructions: moves between general and SSE registers, scalar float arithmetic,
//! comparisons and conversions. Each has its mandatory prefix before REX, then `0F` and its
//! opcode.

use super::form::Kind::{R, RM, RMn, X, XMn};
use super::form::{Form, Kind, Layout, Opcode, Prefix, Sizes, W};
use crate::reg::Size;

const X_SRC: Layout = Layout::RegRm { reg: 0, rm: 1 };
const X_DST: Layout = Layout::RegRm { reg: 1, rm: 0 };

/// An SSE register and a single-precision float in an SSE register or memory.
pub(super) const SS: &[Kind] = &[X, XMn(Size::B32)];
/// An SSE register and a double-precision float in an SSE register or memory.
pub(super) const SD: &[Kind] = &[X, XMn(Size::B64)];

/// `movd` and `movq` between a general register or memory and an SSE register, by width.
pub(super) const MOVD: [Form; 4] = [
    Form::new(&[X, RMn(Size::B32)], Sizes::of(&[Size::B32]), Opcode::two(0x6e), X_SRC)
        .prefix(Prefix::P66)
        .w(W::Never),
    Form::new(&[RMn(Size::B32), X], Sizes::of(&[Size::B32]), Opcode::two(0x7e), X_DST)
        .prefix(Prefix::P66)
        .w(W::Never),
    Form::new(&[X, RMn(Size::B64)], Sizes::of(&[Size::B64]), Opcode::two(0x6e), X_SRC)
        .prefix(Prefix::P66)
        .w(W::Always)
        .named("movq"),
    Form::new(&[RMn(Size::B64), X], Sizes::of(&[Size::B64]), Opcode::two(0x7e), X_DST)
        .prefix(Prefix::P66)
        .w(W::Always)
        .named("movq"),
];

/// A scalar operation, `x = x op xm`, by precision: `single` at 32 bits, `double` at 64, as
/// their prefixes and mnemonics say.
pub(super) const fn scalar(
    opcode: u8,
    single: (Prefix, &'static str),
    double: (Prefix, &'static str),
) -> [Form; 2] {
    [
        Form::new(SS, Sizes::of(&[Size::B32]), Opcode::two(opcode), X_SRC)
            .prefix(single.0)
            .w(W::Never)
            .named(single.1),
        Form::new(SD, Sizes::of(&[Size::B64]), Opcode::two(opcode), X_SRC)
            .prefix(double.0)
            .w(W::Never)
            .named(double.1),
    ]
}

/// One precision's operation, `x = x op xm`.
const fn one(prefix: Prefix, opcode: u8, kinds: &'static [Kind]) -> [Form; 1] {
    [Form::new(kinds, Sizes::ALL, Opcode::two(opcode), X_SRC).prefix(prefix).w(W::Never)]
}

pub(super) const CVTSS2SD: [Form; 1] = one(Prefix::F3, 0x5a, SS);
pub(super) const CVTSD2SS: [Form; 1] = one(Prefix::F2, 0x5a, SD);

/// A signed integer of the instruction's size to a float of `prefix`'s precision.
const fn from_int(prefix: Prefix) -> [Form; 1] {
    [Form::new(&[X, RM], Sizes::WIDE, Opcode::two(0x2a), X_SRC).prefix(prefix).w(W::Rex)]
}

/// A float of `prefix`'s precision, in the second of `kinds`, truncated to a signed integer.
const fn to_int(prefix: Prefix, kinds: &'static [Kind]) -> [Form; 1] {
    [Form::new(kinds, Sizes::WIDE, Opcode::two(0x2c), X_SRC).prefix(prefix).w(W::Rex)]
}

pub(super) const CVTSI2SS: [Form; 1] = from_int(Prefix::F3);
pub(super) const CVTSI2SD: [Form; 1] = from_int(Prefix::F2);
pub(super) const CVTTSS2SI: [Form; 1] = to_int(Prefix::F3, &[R, XMn(Size::B32)]);
pub(super) const CVTTSD2SI: [Form; 1] = to_int(Prefix::F2, &[R, XMn(Size::B64)]);
