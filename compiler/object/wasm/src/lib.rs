//! WebAssembly modules as data (spec 13, section 8.4): the facts of the encoding the wasm backend
//! writes objects in and the wasm linker writes programs in, one declaration each, and
//! relocatable objects in the tool conventions' format, read back ([`Object::read`]).

mod encode;
pub mod format;
mod input;
mod object;
mod read;
#[cfg(test)]
mod tests;

pub use encode::Wasm;
pub use format::{
    ENV, External, FuncType, Import, Limits, PAGE, RelocKind, Section, SymbolFlags, SymbolKind, ValType,
};
pub use input::{Coded, Malformed};
pub use object::{Code, Comdat, Data, DataDef, Init, Object, ObjectImport, Reloc, Segment, Symbol, Symbolic};
