//! What typing finds (spec 02 and 04): the types of a body's expressions, why it gave some none,
//! and the queries typing each instantiation and declaration, which `vun-sema-typeck` computes.
//! Evaluation reads them without depending on the code that finds them: sema defines the
//! interface, the driver installs the typer.

mod found;
mod gave_up;
mod operation;
mod query;
mod reported;

pub use found::{Found, GaveUp, Step, TypedBody};
pub use gave_up::{GaveUps, Leaves};
pub use operation::{Iteration, Operation, Operator};
pub use query::{Enclosing, Typed, Typist, Typists};
pub use reported::{Reads, Reported};

use vun_codec::{Decode, Encode};
use vun_sema_ty::Ty;

/// When a value is known: at compile time, or only at run time. Only run-time integers are
/// capped at 128 bits (spec 04, rule 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Encode, Decode)]
pub enum Stage {
    Compile,
    Run,
}

impl Stage {
    /// When it runs, as dumps and editors show it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Compile => "compile",
            Self::Run => "run",
        }
    }
}

/// A typed expression.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Known {
    pub ty: Ty,
    pub stage: Stage,
}
