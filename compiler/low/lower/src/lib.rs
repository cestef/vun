//! Lowering a program's MIR to LIR (spec 13, section 3): the functions its exports reach, each
//! from its concrete MIR, with range integers in their storage, aggregates and enums in memory
//! by their shapes, and trap sites calling `lang.panic.trap`.

mod func;
mod program;
mod repr;
mod signature;
mod unlowered;

pub use program::Lowered;
pub use repr::{Repr, Types};
pub use unlowered::{Form, Unlowered, Why};
