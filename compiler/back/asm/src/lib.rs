//! What every assembler of the toolchain shares, whatever the architecture: a code buffer that
//! instructions are appended to, labels bound in it and the fields referring to them patched once
//! the function ends, relocations for what the linker places, and the listing of what was
//! emitted. An instruction set (`vun-isa`) says how an instruction is listed and a label reference
//! written; its assembler (`vun-asm-x86_64`, `vun-asm-arm64`) emits into a [`Buffer`].

mod buffer;
mod listing;

#[cfg(test)]
mod tests;

pub use buffer::{Buffer, Code, Reloc};
pub use vun_isa::{AsmOperand, Blocks, Isa, Label, Place, Unpatched, Untaken, Use, Via, imm};
pub use listing::{Line, Listing};
pub use vun_object::RelocKind;
