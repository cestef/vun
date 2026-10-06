//! One backend compiling the functions another cannot express, for the same object: a backend
//! without inline assembly or naked functions hands those to one that has them. The helper's code
//! is placed beside its own, calling the same convention, so a function of either calls the
//! other.

use vun_backend::{Lack, Request};
use vun_lir::FuncId;
use vun_target::TargetSpec;

use crate::Compiled;

/// The code of functions, each by its id.
pub type Assisted = Result<Vec<(FuncId, Compiled<()>)>, Lack>;

pub trait Assist: Send + Sync {
    /// Whether it compiles for `target`.
    fn assists(&self, target: &TargetSpec) -> bool;

    /// The code of the functions `ids` of the request's unit.
    ///
    /// # Errors
    /// Returns what it cannot compile either.
    fn compile(&self, request: &Request<'_>, ids: &[FuncId]) -> Assisted;
}
