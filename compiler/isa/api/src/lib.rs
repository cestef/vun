//! What an instruction set is, whatever the architecture: an instruction as its assembler emits
//! it and how a reference to a label is written ([`Isa`]), labels, how an operand reaches what the
//! linker places, the arithmetic of immediates, and what it reads of blocks of inline assembly
//! ([`Blocks`]). The instruction sets (`vun-isa-x86_64`, `vun-isa-arm64`) are data and pure
//! functions over it, which sema checks blocks against; the assemblers (`vun-asm`) emit them.

mod blocks;
mod fit;
pub mod imm;
mod width;
pub mod word;

use std::fmt;

pub use blocks::{AsmOperand, Blocks, Place, Use};
pub use fit::{ChoiceId, Fit, Fixed, Given, Misfit, Position, Takes, Why, Writeback, positions, width};
pub use width::Width;

/// A place in a function's code that jumps name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Label(u32);

impl Label {
    /// The `number`th label of a function's code, as its buffer makes it.
    #[must_use]
    pub const fn new(number: u32) -> Self {
        Self(number)
    }

    #[must_use]
    pub const fn number(self) -> u32 {
        self.0
    }
}

impl fmt::Display for Label {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, ".L{}", self.0)
    }
}

/// Why a reference to a label could not be written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unpatched {
    /// The label is never bound.
    Unbound(Label),
    /// The label is further than the field reaches.
    Range { at: u64, distance: i64 },
}

impl fmt::Display for Unpatched {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unbound(label) => write!(f, "the label {label} is never bound"),
            Self::Range { at, distance } => {
                write!(f, "the jump at {at:#x} spans {distance} bytes, further than it reaches")
            }
        }
    }
}

impl std::error::Error for Unpatched {}

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
