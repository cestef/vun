//! The System V AMD64 convention's registers (spec 13, section 8.5): `vun-abi` places each value
//! of a call by its rules. Integers and addresses travel in general registers, floats in SSE
//! ones, and a small record's eightbytes in either, by their class.

use vun_asm_x86_64::{Reg, Xmm};

pub use vun_asm_x86_64::conventions::{ARGS, RETURNS, SSE_ARGS, SSE_RETURNS, SYSV};

/// A register a value travels in: a general one, or an SSE one for floats.
pub type Carrier = vun_abi::Carrier<Reg, Xmm>;
/// Where one LIR value of a signature travels.
pub type Loc = vun_abi::Loc<Carrier>;
/// How a result comes back.
pub type Ret = vun_abi::Ret<Carrier>;
/// Where a call of a signature passes everything.
pub type Passing = vun_abi::Passing<Carrier>;
