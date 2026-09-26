//! Analyses every backend reads (spec 13, section 8.1), computed once per function: the control
//! flow graph, the loop forest, the order blocks are emitted in, the live range of each value and
//! the moves filling a jump's target parameters; over a whole unit, the worst-case stack use of
//! each function.

mod cfg;
mod liveness;
mod loops;
mod moves;
mod order;
mod reach;
mod stack;

pub use cfg::Cfg;
pub use liveness::{Liveness, Range};
pub use loops::Loops;
pub use moves::{Location, Move, Moves};
pub use order::Order;
pub use reach::Reached;
pub use stack::{StackUse, Unbounded};

#[cfg(test)]
mod tests;
