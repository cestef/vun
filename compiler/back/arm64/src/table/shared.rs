//! The operand kinds and the width encodings the tables share.

use super::form::Sized;
use super::kind::{Cond, Imm, Kind, R31, Shifts, Width};
use crate::reg::Size::{B8, B16, B32, B64};

/// A general register of the instruction's width, 31 the zero register.
pub(super) const R: Kind = Kind::Gpr(R31::Zr, Width::Inst);
/// A general register of the instruction's width, 31 the stack pointer.
pub(super) const RSP: Kind = Kind::Gpr(R31::Sp, Width::Inst);
pub(super) const W: Kind = Kind::Gpr(R31::Zr, Width::Is(B32));
pub(super) const X: Kind = Kind::Gpr(R31::Zr, Width::Is(B64));
/// A floating-point register of the instruction's width.
pub(super) const F: Kind = Kind::Fpr(Width::Inst);
pub(super) const FS: Kind = Kind::Fpr(Width::Is(B32));
pub(super) const FD: Kind = Kind::Fpr(Width::Is(B64));
pub(super) const SHIFTED: Kind = Kind::Shifted(Shifts::Arith);
pub(super) const LOGIC: Kind = Kind::Shifted(Shifts::Logic);
pub(super) const ARITH: Kind = Kind::Imm(Imm::Arith);
pub(super) const BITMASK: Kind = Kind::Imm(Imm::Bitmask);
pub(super) const COND: Kind = Kind::Cond(Cond::Operand);
pub(super) const INVERTED: Kind = Kind::Cond(Cond::Inverted);

/// The condition field of a conditional select.
pub(super) const CC: u8 = 12;

/// `sf`: 64 bits sets bit 31.
pub(super) const SF: &[Sized] = &[Sized::new(B32, 0), Sized::new(B64, 1 << 31)];
/// `sf` and `N`, which the bitfield instructions set together.
pub(super) const SF_N: &[Sized] = &[Sized::new(B32, 0), Sized::new(B64, 1 << 31 | 1 << 22)];
/// 64 bits only, the base word holding it.
pub(super) const X64: &[Sized] = &[Sized::new(B64, 0)];
/// 32 or 64 bits encoded alike: writing a 32-bit register clears the upper half.
pub(super) const WORDS: &[Sized] = &[Sized::new(B32, 0), Sized::new(B64, 0)];
/// Every width, none encoded: branches.
pub(super) const ANY: &[Sized] =
    &[Sized::new(B8, 0), Sized::new(B16, 0), Sized::new(B32, 0), Sized::new(B64, 0)];
/// `ftype`: single or double precision.
pub(super) const FTYPE: &[Sized] = &[Sized::new(B32, 0), Sized::new(B64, 1 << 22)];
/// `sf` and `ftype` together: moves between general and floating-point registers.
pub(super) const SF_FTYPE: &[Sized] = &[Sized::new(B32, 0), Sized::new(B64, 1 << 31 | 1 << 22)];
/// `size`, the width of an access: bytes and halves suffix the mnemonic.
pub(super) const ACCESS: &[Sized] = &[
    Sized::new(B8, 0).suffix("b"),
    Sized::new(B16, 1 << 30).suffix("h"),
    Sized::new(B32, 2 << 30),
    Sized::new(B64, 3 << 30),
];
/// `size` of a load sign-extending to 64 bits, which the mnemonic's suffix names.
pub(super) const SIGNED: &[Sized] = &[
    Sized::new(B8, 0).suffix("b"),
    Sized::new(B16, 1 << 30).suffix("h"),
    Sized::new(B32, 2 << 30).suffix("w"),
];
/// `size` of a floating-point access.
pub(super) const FP_ACCESS: &[Sized] = &[Sized::new(B32, 2 << 30), Sized::new(B64, 3 << 30)];
/// `opc` of a pair of general registers.
pub(super) const PAIRS: &[Sized] = &[Sized::new(B32, 0), Sized::new(B64, 2 << 30)];
/// `opc` of a pair of floating-point registers.
pub(super) const FP_PAIRS: &[Sized] = &[Sized::new(B32, 0), Sized::new(B64, 1 << 30)];
