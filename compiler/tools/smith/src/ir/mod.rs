//! The programs the smith makes, as the model knows them: typed forms whose types follow the
//! spec's rules, so that what the generator builds is well typed, and what prints from them
//! says what the model says.

mod build;
mod expr;
mod fold;
mod place;
mod program;
mod stmt;

pub use build::Untyped;
pub use expr::{
    Arith, Arm, Capture, Capturing, Closure, Compare, Convert, Expr, Kind, Logic, Narrow, Pattern, Unary,
};
pub use fold::{Fold, Piece, Step, WORD};
pub use place::{Place, Projection};
pub use program::{Bound, FnId, Function, Local, LocalId, Program, Role, TypeId};
pub use stmt::{Assign, Binding, Label, Stmt};
