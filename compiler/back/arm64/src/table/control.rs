//! Branches, calls, traps, barriers and page addresses (Arm A64 ISA, branches, exception
//! generation and system instructions).

use super::form::{D, Form, N};
use super::kind::{Branch, Cond, Imm, Kind};
use super::shared::{ANY, R, SF, X};

pub(super) const B: [Form; 1] = [Form::new(0x1400_0000, ANY, &[Kind::Target(Branch::Jump26).at(0)])];
pub(super) const BL: [Form; 1] = [Form::new(0x9400_0000, ANY, &[Kind::Target(Branch::Call26).at(0)])];
pub(super) const BCOND: [Form; 1] =
    [Form::new(0x5400_0000, ANY, &[Kind::Cond(Cond::Suffix).at(0), Kind::Target(Branch::Cond19).at(N)])];
pub(super) const CBZ: [Form; 1] =
    [Form::new(0x3400_0000, SF, &[R.at(D), Kind::Target(Branch::Cond19).at(N)])];
pub(super) const CBNZ: [Form; 1] =
    [Form::new(0x3500_0000, SF, &[R.at(D), Kind::Target(Branch::Cond19).at(N)])];
pub(super) const BR: [Form; 1] = [Form::new(0xd61f_0000, ANY, &[X.at(N)])];
pub(super) const BLR: [Form; 1] = [Form::new(0xd63f_0000, ANY, &[X.at(N)])];
/// Returns to the link register.
pub(super) const RET: [Form; 1] = [Form::new(0xd65f_03c0, ANY, &[])];
pub(super) const UDF: [Form; 1] = [Form::new(0x0000_0000, ANY, &[Kind::Imm(Imm::U16).at(0)])];
pub(super) const BRK: [Form; 1] = [Form::new(0xd420_0000, ANY, &[Kind::Imm(Imm::U16).at(N)])];
pub(super) const DMB_ISH: [Form; 1] = [Form::new(0xd503_3bbf, ANY, &[])];
pub(super) const ADRP: [Form; 1] = [Form::new(0x9000_0000, ANY, &[X.at(D), Kind::Page.at(0)])];
