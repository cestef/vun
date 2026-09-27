//! What every assembler of the toolchain shares, whatever the architecture: a code buffer that
//! instructions are appended to, labels bound in it and the fields referring to them patched once
//! the function ends, relocations for what the linker places, the listing of what was emitted,
//! and the arithmetic of immediates. An architecture (`vun-asm-x86_64`, `vun-asm-arm64`) declares
//! its instructions and how a label reference is written ([`Isa`]), and emits into a
//! [`Buffer`].

mod buffer;
pub mod imm;
mod inline;
mod listing;

#[cfg(test)]
mod tests;

pub use buffer::{Buffer, Code, Label, Reloc, Unpatched};
pub use inline::{AsmOperand, Blocks, Place};
pub use listing::{Line, Listing};
pub use vun_object::RelocKind;

/// An instruction set, as its assembler emits it.
pub trait Isa {
    /// An instruction as emitted, which the listing prints; `S` names what the linker places.
    type Inst<S: Copy>: Copy;
    /// How a reference to a label is written into the code: its width and what it counts.
    type Fixup: Copy;

    /// Writes into `code` the distance from the reference recorded at `at` to `target`, both
    /// offsets in the code.
    ///
    /// # Errors
    /// Returns when the distance does not fit the field.
    fn patch(fixup: Self::Fixup, code: &mut [u8], at: u64, target: u64) -> Result<(), Unpatched>;
}

/// Operands the form an instruction was checked against does not encode at its width: an
/// immediate none of its encodings holds, or a register its field cannot name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Untaken;

impl std::fmt::Display for Untaken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the form does not encode these operands at this width")
    }
}

impl std::error::Error for Untaken {}

/// How an operand reaches something the linker places.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Via {
    /// Its own address.
    Direct,
    /// Its entry in the global offset table, for something defined elsewhere.
    Got,
}
