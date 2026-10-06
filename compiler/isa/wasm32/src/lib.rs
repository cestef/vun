//! WebAssembly, the instruction set of the `wasm32` targets. Every instruction is declared once,
//! in `lang.arch.wasm32`, as its forms, from which `generated` is made: an opcode, the operands
//! (bound locals and immediates), and the values it pops and pushes. Sema checks blocks of inline
//! assembly against the same forms, and the wasm backend encodes the instructions a block holds.

mod fit;
#[rustfmt::skip]
mod generated;
mod inline;
mod reg;
mod table;

#[cfg(test)]
mod tests;

pub use reg::Size;
pub use table::{
    Access, Arg, Choice, Class, Control, Feature, Flow, Form, Global, Imm, Kind, Need, Op, Processor, Ty,
    Yields,
};

/// The instruction set.
#[derive(Clone, Copy, Debug)]
pub struct Wasm32;
