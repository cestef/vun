//! Scalar floating-point instructions (Arm A64 ISA, scalar floating-point data processing): the
//! instruction's width is the precision, or for a conversion the integer's width.

use super::form::{Arg, D, Form, M, N, Sized};
use super::kind::{Imm, Kind};
use super::shared::{F, FD, FS, FTYPE, R, SF, SF_FTYPE};
use crate::reg::Size;

const F2: &[Arg] = &[F.at(D), F.at(N)];
const F3: &[Arg] = &[F.at(D), F.at(N), F.at(M)];

/// Two operands into a third.
pub(super) const fn binary(base: u32) -> [Form; 1] {
    [Form::new(base, FTYPE, F3)]
}

pub(super) const FADD: u32 = 0x1e20_2800;
pub(super) const FSUB: u32 = 0x1e20_3800;
pub(super) const FMUL: u32 = 0x1e20_0800;
pub(super) const FDIV: u32 = 0x1e20_1800;

/// One operand into another.
pub(super) const fn unary(base: u32) -> [Form; 1] {
    [Form::new(base, FTYPE, F2)]
}

pub(super) const FNEG: u32 = 0x1e21_4000;
pub(super) const FABS: u32 = 0x1e20_c000;
pub(super) const FSQRT: u32 = 0x1e21_c000;

pub(super) const FCMP: [Form; 2] = [
    Form::new(0x1e20_2000, FTYPE, &[F.at(N), F.at(M)]),
    Form::new(0x1e20_2008, FTYPE, &[F.at(N), Kind::Imm(Imm::Zero).at(M)]),
];

/// Between floating-point registers, then from a general register, then to one.
pub(super) const FMOV: [Form; 3] = [
    Form::new(0x1e20_4000, FTYPE, F2),
    Form::new(0x1e27_0000, SF_FTYPE, &[F.at(D), R.at(N)]),
    Form::new(0x1e26_0000, SF_FTYPE, &[R.at(D), F.at(N)]),
];

/// To the instruction's precision from the other.
pub(super) const FCVT: [Form; 2] = [
    Form::new(0x1e22_c000, &[Sized::new(Size::B64, 0)], &[F.at(D), FS.at(N)]),
    Form::new(0x1e62_4000, &[Sized::new(Size::B32, 0)], &[F.at(D), FD.at(N)]),
];

const FS_R: &[Arg] = &[FS.at(D), R.at(N)];
const FD_R: &[Arg] = &[FD.at(D), R.at(N)];
const R_FS: &[Arg] = &[R.at(D), FS.at(N)];
const R_FD: &[Arg] = &[R.at(D), FD.at(N)];

/// An integer of the instruction's width to single or double precision.
pub(super) const fn from_int(base: u32) -> [[Form; 1]; 2] {
    [[Form::new(base, SF, FS_R)], [Form::new(base | DOUBLE, SF, FD_R)]]
}

/// Single or double precision to an integer of the instruction's width, rounded toward zero.
pub(super) const fn to_int(base: u32) -> [[Form; 1]; 2] {
    [[Form::new(base, SF, R_FS)], [Form::new(base | DOUBLE, SF, R_FD)]]
}

/// `ftype` of double precision.
const DOUBLE: u32 = 1 << 22;

pub(super) const SCVTF: u32 = 0x1e22_0000;
pub(super) const UCVTF: u32 = 0x1e23_0000;
pub(super) const FCVTZS: u32 = 0x1e38_0000;
pub(super) const FCVTZU: u32 = 0x1e39_0000;
