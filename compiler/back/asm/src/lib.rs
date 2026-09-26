//! What every assembler of the toolchain shares, whatever the architecture: a code buffer that
//! instructions are appended to, labels bound in it and the fields referring to them patched once
//! the function ends, relocations for what the linker places, the listing of what was emitted,
//! and the arithmetic of immediates. An architecture (`vun-asm-x86_64`, `vun-asm-arm64`) declares
//! its instructions and how a label reference is written ([`Isa`]), and emits into a
//! [`Buffer`].

mod buffer;
pub mod imm;
mod listing;

#[cfg(test)]
mod tests;

pub use buffer::{Buffer, Code, Label, Reloc, Unpatched};
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

/// How an operand reaches something the linker places.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Via {
    /// Its own address.
    Direct,
    /// Its entry in the global offset table, for something defined elsewhere.
    Got,
}
