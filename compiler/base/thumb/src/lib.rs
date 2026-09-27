//! Thumb, the instruction set of Arm's M profile. Its instructions are declared once, in
//! `lang.arch.thumb`, as their forms, from which `generated` is made. No native backend emits
//! Thumb, so the table holds no encodings: sema checks blocks against its forms, and the C backend
//! writes them through their listing.

pub mod conventions;
mod fit;
#[rustfmt::skip]
mod generated;
mod inline;
mod inst;
mod operand;
mod reg;
mod table;

#[cfg(test)]
mod tests;

pub use inst::Inst;
pub use operand::Operand;
pub use reg::{Barrier, Cc, Choice, Class, Mask, Reg, Size, Special};
pub use table::{Access, Form, Kind, Op, Slot};

/// The instruction set.
#[derive(Clone, Copy, Debug)]
pub struct Thumb;
