//! Mid-level IR: a control-flow graph over places (spec 13, section 3).

pub mod body;
pub mod build;
pub mod concrete;
pub mod place;
pub mod program;
pub mod rvalue;
pub mod text;
mod types;
pub mod verify;

pub use body::{Block, BlockId, Body, Local, LocalDecl, Role, Stmt, Terminator, TerminatorKind};
pub use build::{Mir, Unbuilt};
pub use concrete::{Concrete, Lowered, Unready};
pub use place::{Place, Projection};
pub use program::{Program, Programs};
pub use rvalue::{Aggregate, Operand, Rvalue};
pub use verify::Malformed;

#[cfg(test)]
mod tests;
