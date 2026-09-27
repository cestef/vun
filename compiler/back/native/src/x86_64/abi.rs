//! The System V AMD64 convention's registers (spec 13, section 8.5): `vun-abi` places each value
//! of a call by its rules. Integers and addresses travel in general registers, floats in SSE
//! ones, and a small record's eightbytes in either, by their class.

use vun_abi::{Convention, Registers, Rules};
use vun_asm_x86_64::{Reg, Xmm};

/// The registers integers and addresses are passed in, in order.
pub const ARGS: [Reg; 6] = [Reg::Rdi, Reg::Rsi, Reg::Rdx, Reg::Rcx, Reg::R8, Reg::R9];
/// The registers a record of two eightbytes is returned in.
pub const RETURNS: [Reg; 2] = [Reg::Rax, Reg::Rdx];
/// The registers floats are passed in, in order.
pub const SSE_ARGS: [Xmm; 8] = [Xmm(0), Xmm(1), Xmm(2), Xmm(3), Xmm(4), Xmm(5), Xmm(6), Xmm(7)];
/// The registers a record's float eightbytes are returned in.
pub const SSE_RETURNS: [Xmm; 2] = [Xmm(0), Xmm(1)];

/// System V AMD64: a result in memory's address is passed first, and given back in `rax`.
pub const SYSV: Convention<Reg, Xmm> = Convention {
    rules: &Rules::SYSV,
    registers: Registers {
        int_args: &ARGS,
        float_args: &SSE_ARGS,
        int_results: &RETURNS,
        float_results: &SSE_RETURNS,
        result_address: None,
        preserved: &[Reg::Rbx, Reg::Rbp, Reg::R12, Reg::R13, Reg::R14, Reg::R15],
        reserved: &[],
    },
};

/// A register a value travels in: a general one, or an SSE one for floats.
pub type Carrier = vun_abi::Carrier<Reg, Xmm>;
/// Where one LIR value of a signature travels.
pub type Loc = vun_abi::Loc<Carrier>;
/// How a result comes back.
pub type Ret = vun_abi::Ret<Carrier>;
/// Where a call of a signature passes everything.
pub type Passing = vun_abi::Passing<Carrier>;
