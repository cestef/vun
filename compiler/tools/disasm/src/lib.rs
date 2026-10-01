//! Machine code read back as text (`vun debug disasm`): the instructions of the code an ELF or PE
//! file holds, each with its address and bytes, labelled by the file's symbols. A format is read
//! into [`Code`] once; an instruction set is a variant of [`Isa`], so reading another is one more;
//! and bytes that are no instruction of it are listed one at a time as data.

mod code;
mod elf;
mod isa;
mod listing;
mod pe;
#[cfg(test)]
mod tests;

pub use code::{Code, Error, Part, Symbol};
pub use isa::{Decoded, Isa};
pub use listing::{Line, Target, listing};
