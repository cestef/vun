//! The procedure call standard for arm64's registers (AAPCS64, and Apple's variant; spec 13,
//! section 8.5): `vun-abi` places each value of a call by its rules. Integers and addresses travel
//! in general registers, floats in floating-point ones, and results come back in the first of
//! each.

use vun_asm_arm64::{Reg, V};

pub use vun_asm_arm64::conventions::{AAPCS64, ARGS, DARWIN_ARM64, FP_ARGS, INDIRECT};

/// A register a value travels in: a general one, or a floating-point one.
pub type Carrier = vun_abi::Carrier<Reg, V>;
/// Where one LIR value of a signature travels.
pub type Loc = vun_abi::Loc<Carrier>;
/// How a result comes back.
pub type Ret = vun_abi::Ret<Carrier>;
/// Bytes of a record travelling in one register.
pub type Part = vun_abi::Part<Carrier>;
/// Where a call of a signature passes everything.
pub type Passing = vun_abi::Passing<Carrier>;
