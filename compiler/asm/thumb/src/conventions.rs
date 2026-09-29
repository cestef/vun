//! The procedure call standard for Thumb's registers (AAPCS, and its variant passing floats in
//! floating-point registers; spec 13, section 8.5), which `vun-abi` places the values of a call
//! in by its rules; native code and the C backend's inline assembly both read them.

use vun_abi::{Access, Convention, Registers, Rules, Thread};

use crate::{Reg, V};

/// The registers integers and addresses are passed in, in order, and a result returned in.
pub const ARGS: [Reg; 4] = [Reg::R0, Reg::R1, Reg::R2, Reg::R3];
/// The registers floats are passed in and returned in, as single-precision ones: a double takes
/// `v{2k}`, which is `d{k}`.
pub const FP_ARGS: [V; 16] = [
    V::V0,
    V::V1,
    V::V2,
    V::V3,
    V::V4,
    V::V5,
    V::V6,
    V::V7,
    V::V8,
    V::V9,
    V::V10,
    V::V11,
    V::V12,
    V::V13,
    V::V14,
    V::V15,
];
/// The registers a call preserves: `r4` to `r11`.
const PRESERVED: [Reg; 8] = [Reg::R4, Reg::R5, Reg::R6, Reg::R7, Reg::R8, Reg::R9, Reg::R10, Reg::R11];

/// AAPCS with floats in floating-point registers.
pub const AAPCS_VFP: Convention<Reg, V> = Convention {
    rules: Rules::of(vun_target::Convention::AapcsVfp).expect("built"),
    registers: Registers {
        int_args: &ARGS,
        float_args: &FP_ARGS,
        int_results: &ARGS,
        float_results: &FP_ARGS,
        result_address: None,
        preserved: &PRESERVED,
        reserved: &[],
    },
    thread: Some(THREAD),
};

/// AAPCS's base standard: floats travel in general registers, so none is passed in a
/// floating-point one.
pub const AAPCS: Convention<Reg, V> = Convention {
    rules: Rules::of(vun_target::Convention::Aapcs).expect("built"),
    registers: Registers { float_args: &[], float_results: &[], ..AAPCS_VFP.registers },
    thread: Some(THREAD),
};

/// The offset of a thread's storage is built in `r0`, the thread pointer read into `r1`.
const THREAD: Thread<Reg> = Thread { access: Access::Offset, address: Reg::R0, overwrites: &[Reg::R1] };

/// The conventions Thumb code follows, by the name targets give them.
pub const CONVENTIONS: &[(vun_target::Convention, &Convention<Reg, V>)] =
    &[(vun_target::Convention::AapcsVfp, &AAPCS_VFP), (vun_target::Convention::Aapcs, &AAPCS)];
