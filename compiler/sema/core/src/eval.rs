//! The interface to compile-time evaluation (spec 13, section 6).
//!
//! Sema defines what it needs from an evaluator; `vun-interp` implements it, and the driver
//! installs it in the query database, so that sema calls it without depending on it. Calls are
//! memoized on the function and its arguments (spec 03, section 7), which also makes `Pair(u16)`
//! one type.

use std::sync::{Arc, OnceLock};

use vun_codec::{Decode, Encode};
use vun_query::{Cx, Cycle, Query};
use vun_sema_ty::call::{Instance, Instantiation};
use vun_sema_ty::trap::Reason;
use vun_sema_ty::trap::Unevaluated;
use vun_sema_ty::{Signature, Value};

use crate::callable::Parameters;
use crate::env::Env;
use crate::layout::Exists;

pub trait Evaluator: Send + Sync {
    /// Runs `call` at compile time, reading what it needs through `cx`, within the step budget
    /// of its function.
    fn call(&self, cx: &Cx<'_>, call: &Instance) -> Metered;
}

/// What a compile-time call gives, and the steps it took: its own and those of the calls it
/// made. Both are functions of the call alone, so a memoized call's outcome does not depend on
/// which evaluation reached it first.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Metered {
    pub value: Result<Value, Unevaluated>,
    pub steps: u64,
    /// By parameter, what a parameter borrowed exclusively holds when the call returns: the
    /// value its caller's loan reaches then; `None` for the other parameters.
    pub written: Vec<Option<Value>>,
}

impl Metered {
    /// A call that did not run: what stopped it.
    #[must_use]
    pub fn unrun(why: Unevaluated) -> Self {
        Self { value: Err(why), steps: 0, written: Vec::new() }
    }
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

/// The stack a compile-time call must have left to start on the one it runs on: enough for
/// what one call computes before it calls another, its body typed and built included.
const RED_ZONE: usize = 1 << 20;
/// The stack the evaluator adds when less is left, on which calls go on nesting.
const SEGMENT: usize = 16 << 20;

/// The signature of an instantiation: its parameter and result types with its `$` parameters
/// bound. Every call of it reads it, so it is computed once.
pub struct Signed;

impl Query for Signed {
    const NAME: &'static str = "signed";
    type Key = Instantiation;
    type Value = Result<Signature, Unevaluated>;

    fn compute(cx: &Cx<'_>, i: &Instantiation) -> Self::Value {
        let inside = Env::inside(cx, &i.f, i.args.iter().cloned()).ok_or(Unevaluated::Absent)?;
        inside.literal_signature(i.f.literal.expr.id, &i.args)
    }

    fn cycle(_: &Cx<'_>, _: &Instantiation, _: &Cycle) -> Self::Value {
        Err(Unevaluated::Cycle)
    }
}

/// A compile-time call, memoized on its function and arguments, with the steps it took. Calls
/// nest as deep as their budget lets them: they run on stack the evaluator adds in segments
/// once the thread's own runs low, never overflowing it (spec 03, section 7).
pub struct MeteredCall;

impl Query for MeteredCall {
    const NAME: &'static str = "metered_call";
    type Key = Instance;
    type Value = Metered;

    fn compute(cx: &Cx<'_>, call: &Instance) -> Self::Value {
        // Every argument is a value of its parameter's type, as at run time.
        let signature = Instantiation::of(cx, call).and_then(|i| i.signature(cx).ok());
        if signature.is_some_and(|s| s.params.len() == call.args.len() && s.refuses(&call.args)) {
            return Metered::unrun(Unevaluated::trap(Reason::Argument));
        }
        let evaluators = cx.db().resource::<Evaluators>();
        match evaluators.0.get() {
            Some(evaluator) => stacker::maybe_grow(RED_ZONE, SEGMENT, || evaluator.call(cx, call)),
            None => Metered::unrun(Unevaluated::Interpreter),
        }
    }

    fn cycle(_: &Cx<'_>, _: &Instance, _: &Cycle) -> Self::Value {
        Metered::unrun(Unevaluated::Cycle)
    }
}

/// The value of a compile-time call.
pub struct Call;

impl Query for Call {
    const NAME: &'static str = "call";
    type Key = Instance;
    type Value = Result<Value, Unevaluated>;

    fn compute(cx: &Cx<'_>, call: &Instance) -> Self::Value {
        cx.get::<MeteredCall>(call).value
    }

    fn cycle(_: &Cx<'_>, _: &Self::Key, _: &Cycle) -> Self::Value {
        Err(Unevaluated::Cycle)
    }
}

/// An instantiation read through queries: the call it runs, its environment and its signature.
pub trait Instantiate: Sized {
    /// The instantiation `call` runs: its function, with its `$` parameters bound to the call's
    /// arguments.
    #[must_use]
    fn of(cx: &Cx<'_>, call: &Instance) -> Option<Self>;

    /// The environment the call `call` runs its body in: its parameters bound, its identity the
    /// call (decision 28), and the types typing found for the instantiation it runs.
    #[must_use]
    fn running<'a, 'db>(cx: &'a Cx<'db>, call: &Instance) -> Option<Env<'a, 'db>>;

    /// The signature of this instantiation, its types evaluated with its `$` parameters bound.
    ///
    /// # Errors
    /// Returns why a parameter or result type has no value.
    fn signature(&self, cx: &Cx<'_>) -> Result<Signature, Unevaluated>;

    /// Whether this instantiation runs only at compile time: its result or a parameter passed at
    /// run time, not a `$` one, is of a type whose values do not exist at run time, such as
    /// `type`, code, an unbounded integer or a borrow of a plan.
    #[must_use]
    fn compile_time(&self, cx: &Cx<'_>) -> bool;
}

impl Instantiate for Instantiation {
    fn of(cx: &Cx<'_>, call: &Instance) -> Option<Self> {
        let comptime = call.f.comptime(cx)?;
        let args = comptime.iter().zip(&call.args).map(|(&c, arg)| c.then(|| arg.clone()));
        Some(Self::new(call.f.clone(), args.collect()))
    }

    fn running<'a, 'db>(cx: &'a Cx<'db>, call: &Instance) -> Option<Env<'a, 'db>> {
        let types = cx.get::<crate::typeck::Typed>(&Self::of(cx, call)?);
        Some(Env::within(cx, call)?.typed_by(types))
    }

    fn signature(&self, cx: &Cx<'_>) -> Result<Signature, Unevaluated> {
        cx.get::<Signed>(self)
    }

    fn compile_time(&self, cx: &Cx<'_>) -> bool {
        let Ok(signature) = self.signature(cx) else { return false };
        let comptime = self.f.comptime(cx).unwrap_or_default();
        let runtime = signature.params.iter().enumerate().filter(|(i, _)| comptime.get(*i) != Some(&true));
        runtime.map(|(_, ty)| ty).chain([&signature.result]).any(|ty| !cx.get::<Exists>(ty))
    }
}
