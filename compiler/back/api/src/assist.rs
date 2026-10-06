//! One backend compiling the functions another cannot express, for the same object: a backend
//! without inline assembly or naked functions hands those to one that has them. The helper's code
//! is placed beside its own, calling the same convention, so a function of either calls the
//! other.

use vun_lir::FuncId;
use vun_target::TargetSpec;

use crate::{Compiled, Lack, Request};

/// The code of functions, each by its id.
pub type Assisted = Result<Vec<(FuncId, Compiled<()>)>, Lack>;

pub trait Assist {
    /// Whether it compiles for `target`.
    fn assists(&self, target: &TargetSpec) -> bool;

    /// The code of the functions `ids` of the request's unit.
    ///
    /// # Errors
    /// Returns what it cannot compile either.
    fn compile(&self, request: &Request<'_>, ids: &[FuncId]) -> Assisted;
}

/// No assistance: what a program that needs none is compiled with.
pub struct Unassisted;

impl Assist for Unassisted {
    fn assists(&self, _: &TargetSpec) -> bool {
        false
    }

    fn compile(&self, _: &Request<'_>, _: &[FuncId]) -> Assisted {
        unreachable!("a backend asks for assistance only where `assists` says it is there")
    }
}
