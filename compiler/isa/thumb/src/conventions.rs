//! The registers the C convention of the M profile passes arguments in (AAPCS, section 6.5):
//! those a block's bindings take where the C backend writes it.

use vun_target::Convention;

use crate::Reg;

/// The general registers integers and addresses are passed in, in order.
pub const ARGS: [Reg; 4] = [Reg::R0, Reg::R1, Reg::R2, Reg::R3];

/// The conventions Thumb code follows, by the name targets give them, with their argument
/// registers.
pub const CONVENTIONS: &[(Convention, &[Reg])] = &[(Convention::AapcsVfp, &ARGS)];
