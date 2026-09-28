//! The x86-64 assembler: each instruction of the instruction set (`vun-isa-x86_64`) encoded by
//! the form its operands fit, into a `vun_asm::Buffer` through [`Emit`]; and the registers of the
//! calling conventions x86-64 code follows.

pub mod conventions;
mod encode;

#[cfg(test)]
mod tests;

pub use encode::Emit;
pub use vun_isa_x86_64::*;

/// A function's code as it is emitted.
pub type Asm<S> = vun_asm::Buffer<X86_64, S>;
