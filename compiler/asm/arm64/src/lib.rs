//! The 64-bit Arm (A64) assembler: each instruction of the instruction set (`vun-isa-arm64`)
//! encoded as the word of the form its operands fit, with the references its fields leave, into a
//! `vun_asm::Buffer` through [`Emit`]; and the registers of the calling conventions arm64 code
//! follows.

pub mod conventions;
mod encode;

#[cfg(test)]
mod tests;

pub use encode::Emit;
pub use vun_isa_arm64::*;

/// A function's code as it is emitted.
pub type Asm<S> = vun_asm::Buffer<Arm64, S>;
