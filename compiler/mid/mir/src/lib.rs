//! Mid-level IR: a control-flow graph over places (spec 13, section 3).

pub mod body;
pub mod build;
mod checked;
pub mod concrete;
pub mod place;
pub mod program;
mod remap;
pub mod rvalue;
pub mod text;
mod types;
pub mod verify;

pub use body::{Block, BlockId, Body, Local, LocalDecl, Role, Stmt, Terminator, TerminatorKind};
pub use build::{Check, Mir, Unbuilt, Why};
pub use concrete::{Concrete, Lowered, Map, Region, Unready};
pub use place::{Place, Projection};
pub use program::{Program, Programs, Reached, Roots, Statement};
pub use rvalue::{Aggregate, Asm, Operand, Rvalue, Stepped};
pub use verify::Malformed;

/// The queries of MIR whose memos outlive the process, with their values.
pub const PERSISTED: &[fn(&vun_query::Db)] =
    &[vun_query::Db::persist::<Mir>, vun_query::Db::persist::<Concrete>, vun_query::Db::persist::<Programs>];

#[cfg(test)]
mod tests;
