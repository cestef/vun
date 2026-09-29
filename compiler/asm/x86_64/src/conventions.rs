//! The System V AMD64 convention's registers (spec 13, section 8.5), which `vun-abi` places the
//! values of a call in by its rules; native code and the C backend's inline assembly both read
//! them.

use vun_abi::{Access, Convention, Registers, Rules, Thread};

use crate::{Reg, Xmm};

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
    rules: Rules::of(vun_target::Convention::Sysv).expect("built"),
    registers: Registers {
        int_args: &ARGS,
        float_args: &SSE_ARGS,
        int_results: &RETURNS,
        float_results: &SSE_RETURNS,
        result_address: None,
        preserved: &[Reg::Rbx, Reg::Rbp, Reg::R12, Reg::R13, Reg::R14, Reg::R15],
        reserved: &[],
    },
    // The call a TLS descriptor names gives the offset in `rax`, keeping every other register.
    thread: Some(Thread { access: Access::Descriptor, address: Reg::Rax, overwrites: &[] }),
};

/// The conventions x86-64 code follows, by the name targets give them.
pub const CONVENTIONS: &[(vun_target::Convention, &Convention<Reg, Xmm>)] =
    &[(vun_target::Convention::Sysv, &SYSV)];
