//! Integer instructions: moves, arithmetic, logic, shifts, extensions and conditional selects
//! (Arm A64 ISA, data processing). An alias is its own instruction, its fixed fields in the base
//! word: `mov` is `orr` from the zero register, `cmp` is `subs` into it.

use super::form::{A, Arg, D, Form, M, N, Sized};
use super::kind::{Imm, Kind};
use super::shared::{ARITH, BITMASK, CC, COND, INVERTED, LOGIC, R, RSP, SF, SF_N, SHIFTED, W, WORDS, X, X64};
use crate::reg::Size::{B8, B16, B32, B64};

/// Register 31 in a fixed field: the zero register, or the stack pointer.
const ZR_D: u32 = 31;
const ZR_N: u32 = 31 << N;
const ZR_M: u32 = 31 << M;
const ZR_A: u32 = 31 << A;

/// Bits 29 and 30 of `add`, `adds`, `sub`, `subs`.
const ADD: u32 = 0;
pub(super) const ADDS: u32 = 1 << 29;
pub(super) const SUB: u32 = 1 << 30;
pub(super) const SUBS: u32 = 3 << 29;

// Operand lists the constructors below share: a constructor's own lists are named, as only
// named constants are promoted to statics within a function.
const R3: &[Arg] = &[R.at(D), R.at(N), R.at(M)];
const R4: &[Arg] = &[R.at(D), R.at(N), R.at(M), R.at(A)];
const R_R_SHIFTED: &[Arg] = &[R.at(D), R.at(N), SHIFTED.at(A)];
const R_R_LOGIC: &[Arg] = &[R.at(D), R.at(N), LOGIC.at(A)];
const CMP_SHIFTED: &[Arg] = &[R.at(N), SHIFTED.at(A)];
const CMP_IMM: &[Arg] = &[RSP.at(N), ARITH.at(A)];
const CMP_EXTENDED: &[Arg] = &[RSP.at(N), Kind::Extended.at(A)];
const NEGATED: &[Arg] = &[R.at(D), SHIFTED.at(A)];
const EXTEND: &[Arg] = &[R.at(D), W.at(N)];
const EXTEND_W: &[Arg] = &[W.at(D), W.at(N)];
const SELECT: &[Arg] = &[R.at(D), R.at(N), R.at(M), COND.at(CC)];
const SET: &[Arg] = &[R.at(D), INVERTED.at(CC)];
const UNARY: &[Arg] = &[R.at(D), R.at(N).again(M), INVERTED.at(CC)];

/// `add`, `adds`, `sub`, `subs` by `op`, over a shifted register, an immediate and an extended
/// register; `imm` and `ext` give the last two forms' operands, whose destination is the stack
/// pointer or, for the flag-setting ones, the zero register.
pub(super) const fn arith(op: u32, imm: &'static [Arg], ext: &'static [Arg]) -> [Form; 3] {
    [
        Form::new(0x0b00_0000 | op, SF, R_R_SHIFTED),
        Form::new(0x1100_0000 | op, SF, imm),
        Form::new(0x0b20_0000 | op, SF, ext),
    ]
}

pub(super) const ADD_FORMS: [Form; 4] = {
    let [shifted, imm, ext] =
        arith(ADD, &[RSP.at(D), RSP.at(N), ARITH.at(A)], &[RSP.at(D), RSP.at(N), Kind::Extended.at(A)]);
    [shifted, imm, ext, Form::new(0x1100_0000, SF, &[RSP.at(D), RSP.at(N), Kind::Lo12.at(A)])]
};

/// `cmp` and `cmn`: `subs` and `adds` into the zero register.
pub(super) const fn compare(op: u32) -> [Form; 3] {
    [
        Form::new(0x0b00_0000 | op | ZR_D, SF, CMP_SHIFTED),
        Form::new(0x1100_0000 | op | ZR_D, SF, CMP_IMM),
        Form::new(0x0b20_0000 | op | ZR_D, SF, CMP_EXTENDED),
    ]
}

/// `neg` and `negs`: `sub` and `subs` from the zero register.
pub(super) const fn negate(op: u32) -> [Form; 1] {
    [Form::new(0x0b00_0000 | op | ZR_N, SF, NEGATED)]
}

/// `opc` of `and`, `orr`, `eor`, `ands`.
pub(super) const AND: u32 = 0;
pub(super) const ORR: u32 = 1 << 29;
pub(super) const EOR: u32 = 2 << 29;
pub(super) const ANDS: u32 = 3 << 29;
/// Negates the shifted register: `bic`, `orn`.
pub(super) const NOT: u32 = 1 << 21;

/// A logical instruction by `opc`, over a shifted register and a bitmask whose operands `imm`
/// gives: its destination is the stack pointer, or for `ands` the zero register.
pub(super) const fn logic(opc: u32, imm: &'static [Arg]) -> [Form; 2] {
    [Form::new(0x0a00_0000 | opc, SF, R_R_LOGIC), Form::new(0x1200_0000 | opc, SF, imm)]
}

/// A logical instruction over a negated shifted register only.
pub(super) const fn negated(opc: u32) -> [Form; 1] {
    [Form::new(0x0a00_0000 | opc | NOT, SF, R_R_LOGIC)]
}

pub(super) const TST: [Form; 2] = [
    Form::new(0x0a00_0000 | ANDS | ZR_D, SF, &[R.at(N), LOGIC.at(A)]),
    Form::new(0x1200_0000 | ANDS | ZR_D, SF, &[R.at(N), BITMASK.at(A)]),
];
pub(super) const MVN: [Form; 1] = [Form::new(0x0a00_0000 | ORR | NOT | ZR_N, SF, &[R.at(D), LOGIC.at(A)])];

/// A register, the stack pointer, then the constants one instruction moves: `movz`, `movn` and
/// a bitmask, in the order a decoder prefers them.
pub(super) const MOV: [Form; 5] = [
    Form::new(0x0a00_0000 | ORR | ZR_N, SF, &[R.at(D), R.at(M)]),
    Form::new(0x1100_0000, SF, &[RSP.at(D), RSP.at(N)]),
    Form::new(0x5280_0000, SF, &[R.at(D), Kind::Imm(Imm::Wide).at(N)]),
    Form::new(0x1280_0000, SF, &[R.at(D), Kind::Imm(Imm::WideNot).at(N)]),
    Form::new(0x1200_0000 | ORR | ZR_N, SF, &[RSP.at(D), BITMASK.at(A)]),
];
pub(super) const MOVK: [Form; 1] = [Form::new(0x7280_0000, SF, &[R.at(D), Kind::Imm(Imm::Half).at(N)])];

pub(super) const MUL: [Form; 1] = [Form::new(0x1b00_0000 | ZR_A, SF, R3)];
pub(super) const MADD: [Form; 1] = [Form::new(0x1b00_0000, SF, R4)];
pub(super) const MSUB: [Form; 1] = [Form::new(0x1b00_8000, SF, R4)];
pub(super) const MNEG: [Form; 1] = [Form::new(0x1b00_8000 | ZR_A, SF, R3)];
pub(super) const SMULH: [Form; 1] = [Form::new(0x9b40_0000 | ZR_A, X64, R3)];
pub(super) const UMULH: [Form; 1] = [Form::new(0x9bc0_0000 | ZR_A, X64, R3)];
pub(super) const SDIV: [Form; 1] = [Form::new(0x1ac0_0c00, SF, R3)];
pub(super) const UDIV: [Form; 1] = [Form::new(0x1ac0_0800, SF, R3)];

/// A shift by a register (`lslv`, by `op2`), or by an immediate as a bitfield move (`ubfm` or
/// `sbfm`, by `bitfield`) whose rotation and top bit `imm` computes.
pub(super) const fn shift(op2: u32, bitfield: u32, imm: &'static [Arg]) -> [Form; 2] {
    [Form::new(0x1ac0_2000 | op2, SF, R3), Form::new(bitfield, SF_N, imm)]
}

pub(super) const UBFM: u32 = 0x5300_0000;
pub(super) const SBFM: u32 = 0x1300_0000;
pub(super) const SHL: &[Arg] = &[R.at(D), R.at(N), Kind::Imm(Imm::Lsl).at(A)];
pub(super) const SHR: &[Arg] = &[R.at(D), R.at(N), Kind::Imm(Imm::Shr).at(A)];

/// `imms`, the top bit a bitfield move extends from.
const IMMS: u8 = 10;

/// A sign extension from bit `top`, to 32 or 64 bits.
pub(super) const fn sign(top: u32) -> [Form; 1] {
    [Form::new(SBFM | top << IMMS, SF_N, EXTEND)]
}

pub(super) const SXTW: [Form; 1] = [Form::new(SBFM | 1 << 31 | 1 << 22 | 31 << IMMS, X64, EXTEND)];

/// A zero extension from bit `top`, which the 32-bit form gives at 64 bits too.
pub(super) const fn zero(top: u32) -> [Form; 1] {
    [Form::new(UBFM | top << IMMS, WORDS, EXTEND_W)]
}

/// The move of a register, which a 64-bit extension from 64 bits and a zero extension from 32
/// bits are: writing a 32-bit register clears the upper half.
const MOV_REG: u32 = 0x0a00_0000 | ORR | ZR_N;
const SXT_SIZES: &[Sized] = &[
    Sized::new(B8, 1 << 31 | 1 << 22 | 7 << IMMS).suffix("b"),
    Sized::new(B16, 1 << 31 | 1 << 22 | 15 << IMMS).suffix("h"),
    Sized::new(B32, 1 << 31 | 1 << 22 | 31 << IMMS).suffix("w"),
];
const UXT_SIZES: &[Sized] = &[Sized::new(B8, 7 << IMMS).suffix("b"), Sized::new(B16, 15 << IMMS).suffix("h")];
const MOV_X: &[Sized] = &[Sized::new(B64, 1 << 31)];

/// A sign extension to 64 bits from the instruction's width.
pub(super) const SXT: [Form; 2] = [
    Form::new(SBFM, SXT_SIZES, &[X.at(D), W.at(N)]).named("sxt"),
    Form::new(MOV_REG, MOV_X, &[R.at(D), R.at(M)]).named("mov"),
];

/// A zero extension to 64 bits from the instruction's width.
pub(super) const UXT: [Form; 2] = [
    Form::new(UBFM, UXT_SIZES, EXTEND_W).named("uxt"),
    Form::new(MOV_REG, SF, &[R.at(D), R.at(M)]).named("mov"),
];

/// A conditional select by `op`.
pub(super) const fn select(op: u32) -> [Form; 1] {
    [Form::new(0x1a80_0000 | op, SF, SELECT)]
}

/// `csinc`, `csinv`, `csneg` in `op`.
pub(super) const CSINC: u32 = 0x400;
pub(super) const CSINV: u32 = 1 << 30;
pub(super) const CSNEG: u32 = 1 << 30 | 0x400;

/// A select of the zero register and itself on the inverse condition: `cset`, `csetm`.
pub(super) const fn set(op: u32) -> [Form; 1] {
    [Form::new(0x1a80_0000 | op | ZR_N | ZR_M, SF, SET)]
}

/// A select of one register and itself on the inverse condition: `cinc`, `cinv`, `cneg`.
pub(super) const fn unary(op: u32) -> [Form; 1] {
    [Form::new(0x1a80_0000 | op, SF, UNARY)]
}
