//! The procedure call standard for Thumb's registers (AAPCS, and its variant passing floats in
//! floating-point registers; spec 13, section 8.5): `vun-abi` places each value of a call by its
//! rules. Integers and addresses travel in general registers, floats in floating-point ones under
//! the variant and in general ones otherwise, and results come back in the first of each.

use vun_asm_thumb::{Reg, V};

pub use vun_asm_thumb::conventions::{AAPCS, AAPCS_VFP, ARGS, FP_ARGS};

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
