//! Loads, stores and atomics (Arm A64 ISA, loads and stores). A single access has a form per
//! addressing mode, tried in order: a scaled offset, an unscaled one (`ldur`), a register index,
//! pre- and post-index, then the low bits of a symbol's address.

use super::form::{Arg, D, Form, M, N};
use super::kind::{Kind, Mode, Width};
use super::shared::{ACCESS, R, W};

/// `opc` of a load, a store, and a load sign-extending to 64 bits.
pub(super) const LOAD: u32 = 1 << 22;
pub(super) const STORE: u32 = 0;
pub(super) const LOAD_SIGNED: u32 = 2 << 22;
/// A floating-point register's access.
pub(super) const FP: u32 = 1 << 26;

/// The forms of a single access in every mode, for each `(opc, register, widths)`; the unscaled
/// one is named `unscaled`.
macro_rules! access {
    ($unscaled:literal: $(($opc:expr, $rt:expr, $sizes:expr)),* $(,)?) => {{
        use $crate::table::{Form, Kind, Mode, Width, form::{D, N}};
        [$(
            Form::new(0x3900_0000 | $opc, $sizes, &[$rt.at(D), Kind::Mem(Mode::Scaled, Width::Inst).at(N)]),
            Form::new(0x3800_0000 | $opc, $sizes, &[$rt.at(D), Kind::Mem(Mode::Unscaled, Width::Inst).at(N)])
                .named($unscaled),
            Form::new(0x3820_0800 | $opc, $sizes, &[$rt.at(D), Kind::Mem(Mode::Index, Width::Inst).at(N)]),
            Form::new(0x3800_0c00 | $opc, $sizes, &[$rt.at(D), Kind::Mem(Mode::Pre, Width::Inst).at(N)]),
            Form::new(0x3800_0400 | $opc, $sizes, &[$rt.at(D), Kind::Mem(Mode::Post, Width::Inst).at(N)]),
            Form::new(0x3900_0000 | $opc, $sizes, &[$rt.at(D), Kind::Mem(Mode::Lo12, Width::Inst).at(N)]),
        )*]
    }};
}

/// The forms of a pair's access at an offset, pre- and post-index, for each
/// `(opc, register, widths)`.
macro_rules! pair {
    ($(($opc:expr, $rt:expr, $sizes:expr)),* $(,)?) => {{
        use $crate::table::{Form, Kind, Mode, Width, form::{A, D, N}};
        [$(
            Form::new(0x2900_0000 | $opc, $sizes, &[$rt.at(D), $rt.at(A), Kind::Mem(Mode::Pair, Width::Inst).at(N)]),
            Form::new(0x2980_0000 | $opc, $sizes, &[$rt.at(D), $rt.at(A), Kind::Mem(Mode::PairPre, Width::Inst).at(N)]),
            Form::new(0x2880_0000 | $opc, $sizes, &[$rt.at(D), $rt.at(A), Kind::Mem(Mode::PairPost, Width::Inst).at(N)]),
        )*]
    }};
}

pub(super) use {access, pair};

/// `L` of a pair's load.
pub(super) const PAIR_LOAD: u32 = 1 << 22;

const BASE: Kind = Kind::Mem(Mode::Base, Width::Inst);
const RT_BASE: &[Arg] = &[R.at(D), BASE.at(N)];
/// The status, the register stored, and the address.
const STATUS_RT_BASE: &[Arg] = &[W.at(M), R.at(D), BASE.at(N)];
/// The register read, the register written, and the address.
const RS_RT_BASE: &[Arg] = &[R.at(M), R.at(D), BASE.at(N)];
const RS_BASE: &[Arg] = &[R.at(M), BASE.at(N)];
/// The zero register as the register transferred.
const ZR_T: u32 = 31;

/// An exclusive or ordered load or store.
pub(super) const fn ordered(base: u32) -> [Form; 1] {
    [Form::new(base, ACCESS, RT_BASE)]
}

pub(super) const LDAR: u32 = 0x08df_fc00;
pub(super) const STLR: u32 = 0x089f_fc00;
pub(super) const LDAXR: u32 = 0x085f_fc00;
pub(super) const LDXR: u32 = 0x085f_7c00;

/// An exclusive store, writing whether it failed.
pub(super) const fn exclusive(base: u32) -> [Form; 1] {
    [Form::new(base, ACCESS, STATUS_RT_BASE)]
}

pub(super) const STXR: u32 = 0x0800_7c00;
pub(super) const STLXR: u32 = 0x0800_fc00;

/// An atomic read-modify-write of the large system extensions.
pub(super) const fn atomic(base: u32) -> [Form; 1] {
    [Form::new(base, ACCESS, RS_RT_BASE)]
}

/// An atomic read-modify-write discarding what memory held: its destination the zero register.
pub(super) const fn discarding(base: u32) -> [Form; 1] {
    [Form::new(base | ZR_T, ACCESS, RS_BASE)]
}

pub(super) const LDADD: u32 = 0x3820_0000;
pub(super) const SWP: u32 = 0x3820_8000;
/// Acquire and release of `ldadd` and `swp`.
pub(super) const ACQUIRE: u32 = 1 << 23;
pub(super) const RELEASE: u32 = 1 << 22;
pub(super) const CAS: u32 = 0x08a0_7c00;
/// Acquire and release of `cas`.
pub(super) const CAS_ACQUIRE: u32 = 1 << 22;
pub(super) const CAS_RELEASE: u32 = 1 << 15;
