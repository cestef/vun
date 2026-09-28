//! The Thumb assembler: each instruction of the instruction set (`vun-isa-thumb`) encoded as the
//! halfword or two of the form its operands fit, with the references its fields leave, into a
//! `vun_asm::Buffer` through [`Emit`]; and the registers of the calling conventions Thumb code
//! follows.

pub mod conventions;
mod encode;

#[cfg(test)]
mod tests;

pub use encode::Emit;
pub use vun_isa_thumb::*;

/// A function's code as it is emitted.
pub type Asm<S> = vun_asm::Buffer<Thumb, S>;
