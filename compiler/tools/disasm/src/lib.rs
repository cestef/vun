//! Machine code read back as text (`vun debug disasm`): the instructions of the code an ELF file
//! holds, each with its address and bytes, labelled by the file's symbols. An instruction set is
//! a row of [`Isa`]'s table, so reading another is one more row, and bytes that are no
//! instruction of it are listed one at a time as data.

mod isa;
mod listing;
#[cfg(test)]
mod tests;

pub use isa::{Decoded, Isa};
pub use listing::{Error, Line, Target, listing};
