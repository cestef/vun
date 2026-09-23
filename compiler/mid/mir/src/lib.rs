//! Mid-level IR: a control-flow graph over places (spec 13, section 3).

pub mod body;
pub mod build;
pub mod place;
pub mod text;

pub use body::{Block, BlockId, Body, Local, LocalDecl, Operand, Rvalue, Stmt, Terminator, TerminatorKind};
pub use build::{Mir, Unbuilt};
pub use place::Place;
