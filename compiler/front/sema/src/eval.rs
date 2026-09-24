//! The interface to compile-time evaluation (spec 13, section 6).
//!
//! Sema defines what it needs from an evaluator; `overt-interp` implements it, and the driver
//! installs it in the query database, so that sema calls it without depending on it. Calls are
//! memoized on the function and its arguments (spec 03, section 7), which also makes `Pair(u16)`
//! one type.

use std::sync::{Arc, OnceLock};

use overt_query::{Cx, Cycle, Query};

use crate::env::Unevaluated;
use crate::item::ExprRef;
use crate::ty::Value;

/// A compile-time call: a function value and its arguments. Types built during the call are
/// identified by it (decision 28), and its body sees its parameters bound to the arguments.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Instance {
    pub f: Func,
    pub args: Vec<Value>,
}

/// A function value: a function literal, and the compile-time call it was evaluated in, whose
/// parameters its body sees too: a method of `Pair(u8)` sees `T` bound to `u8`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Func {
    pub literal: ExprRef,
    pub within: Option<Arc<Instance>>,
}

pub trait Evaluator: Send + Sync {
    /// Runs `call` at compile time, reading what it needs through `cx`.
    ///
    /// # Errors
    /// Returns why the call has no value.
    fn call(&self, cx: &Cx<'_>, call: &Instance) -> Result<Value, Unevaluated>;
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
    type Key = Instance;
    type Value = Result<Value, Unevaluated>;

    fn compute(cx: &Cx<'_>, call: &Instance) -> Self::Value {
        let evaluators = cx.db().resource::<Evaluators>();
        let evaluator = evaluators.0.get().ok_or(Unevaluated::Interpreter)?;
        evaluator.call(cx, call)
    }

    fn cycle(_: &Cx<'_>, _: &Self::Key, _: &Cycle) -> Self::Value {
        Err(Unevaluated::Cycle)
    }
}
