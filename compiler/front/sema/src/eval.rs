//! The interface to compile-time evaluation (spec 13, section 6).
//!
//! Sema defines what it needs from an evaluator; `overt-interp` implements it, and the driver
//! installs it in the query database, so that sema calls it without depending on it. Calls are
//! memoized on the function and its arguments (spec 03, section 7), which also makes `Pair(u16)`
//! one type.

use std::sync::{Arc, OnceLock};

use overt_hir::db::FileRef;
use overt_hir::tree::ExprId;
use overt_query::{Cx, Cycle, Query};

use crate::env::Unevaluated;
use crate::ty::Value;

/// A function literal, by the file and expression that write it: a function value's identity.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FnRef {
    pub file: FileRef,
    pub expr: ExprId,
}

pub trait Evaluator: Send + Sync {
    /// Calls `f` with `args` at compile time, reading what it needs through `cx`.
    ///
    /// # Errors
    /// Returns why the call has no value.
    fn call(&self, cx: &Cx<'_>, f: &FnRef, args: &[Value]) -> Result<Value, Unevaluated>;
}

/// The installed evaluator: a resource of the query database, set once by the driver.
#[derive(Default)]
pub struct Evaluators(OnceLock<Arc<dyn Evaluator>>);

impl Evaluators {
    /// Installs `evaluator`, unless one is installed.
    pub fn install(&self, evaluator: Arc<dyn Evaluator>) {
        let _ = self.0.set(evaluator);
    }
}

/// A compile-time call, memoized on its function and arguments.
pub struct Call;

impl Query for Call {
    const NAME: &'static str = "call";
    type Key = (FnRef, Vec<Value>);
    type Value = Result<Value, Unevaluated>;

    fn compute(cx: &Cx<'_>, (f, args): &Self::Key) -> Self::Value {
        let evaluators = cx.db().resource::<Evaluators>();
        let evaluator = evaluators.0.get().ok_or(Unevaluated::Interpreter)?;
        evaluator.call(cx, f, args)
    }

    fn cycle(_: &Cx<'_>, _: &Self::Key, _: &Cycle) -> Self::Value {
        Err(Unevaluated::Cycle)
    }
}
