//! The procedure call standard for arm64's registers (AAPCS64, and Apple's variant; spec 13,
//! section 8.5): `vun-abi` places each value of a call by its rules. Integers and addresses travel
//! in general registers, floats in floating-point ones, and results come back in the first of
//! each.

use vun_abi::{Convention, Registers, Rules};
use vun_asm_arm64::{Reg, V};

/// The registers integers and addresses are passed in, in order, and a result returned in.
pub const ARGS: [Reg; 8] = [Reg::X0, Reg::X1, Reg::X2, Reg::X3, Reg::X4, Reg::X5, Reg::X6, Reg::X7];
/// The registers floats are passed in, in order, and a result returned in.
pub const FP_ARGS: [V; 8] = [V(0), V(1), V(2), V(3), V(4), V(5), V(6), V(7)];
/// The register the address a result is written through is passed in.
pub const INDIRECT: Reg = Reg::X8;
/// The registers a call preserves: `x19` to `x28`, and the frame pointer.
const PRESERVED: [Reg; 11] = [
    Reg::X19,
    Reg::X20,
    Reg::X21,
    Reg::X22,
    Reg::X23,
    Reg::X24,
    Reg::X25,
    Reg::X26,
    Reg::X27,
    Reg::X28,
    Reg::X29,
];

const REGISTERS: Registers<Reg, V> = Registers {
    int_args: &ARGS,
    float_args: &FP_ARGS,
    int_results: &ARGS,
    float_results: &FP_ARGS,
    result_address: Some(INDIRECT),
    preserved: &PRESERVED,
    reserved: &[],
};

/// AAPCS64.
pub const AAPCS64: Convention<Reg, V> = Convention { rules: &Rules::AAPCS64, registers: REGISTERS };

/// Apple's variant, which also keeps `x18` for the platform.
pub const DARWIN_ARM64: Convention<Reg, V> =
    Convention { rules: &Rules::DARWIN_ARM64, registers: Registers { reserved: &[Reg::X18], ..REGISTERS } };

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
