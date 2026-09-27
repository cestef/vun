//! The interface to compile-time evaluation (spec 13, section 6).
//!
//! Sema defines what it needs from an evaluator; `vun-interp` implements it, and the driver
//! installs it in the query database, so that sema calls it without depending on it. Calls are
//! memoized on the function and its arguments (spec 03, section 7), which also makes `Pair(u16)`
//! one type.

use std::sync::{Arc, OnceLock};

use vun_codec::{Decode, Encode};
use vun_desugar::ITERATION;
use vun_hir::tree::BinderId;
use vun_query::{Cx, Cycle, Query};

use crate::env::{Env, Unevaluated};
use crate::item::{ExprRef, Node};
use crate::layout::Exists;
use crate::trap::Reason;
use crate::ty::{Signature, Ty, Value};

/// A compile-time call: a function value and its arguments. Types built during the call are
/// identified by it (decision 28), and its body sees its parameters bound to the arguments.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Instance {
    pub f: Func,
    pub args: Vec<Value>,
}

/// A function value: a function literal, and the compile-time call it was evaluated in, whose
/// parameters its body sees too: a method of `Pair(u8)` sees `T` bound to `u8`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Func {
    pub literal: ExprRef,
    pub within: Option<Arc<Instance>>,
    /// The `$for` bindings of the entries that generated it, which its body sees too.
    pub staged: Arc<[Staging]>,
}

/// A code value (spec 03, section 4): an expression, and what it was written under, where its
/// names resolve (hygiene by construction): the compile-time call and every binder bound there.
/// A quote `#{ e }` is one, and so is an argument for a `Code(T)` parameter.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Code {
    pub expr: ExprRef,
    pub within: Option<Arc<Instance>>,
    pub bound: Arc<[Staging]>,
}

/// A coroutine's frame at compile time (spec 06): its call, the block of its body it resumes at
/// (`None` once it returned), and its locals.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Suspended {
    pub call: Instance,
    pub at: Option<usize>,
    pub locals: Vec<Option<Value>>,
}

/// What resuming a frame gives: an item it yields, or what it returns.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Resumed {
    Yielded(Value),
    Returned(Value),
}

impl Resumed {
    /// This outcome as a value of `step`, the type resuming gives: its variant `item` or `done`
    /// (the table `iteration`).
    #[must_use]
    pub fn value(self, step: Ty) -> Option<Value> {
        let row = ITERATION.first()?;
        let (name, value) = match self {
            Self::Yielded(value) => (row.each, value),
            Self::Returned(value) => (row.done, value),
        };
        Some(Value::Variant { ty: step, name: name.symbol(), payload: vec![value] })
    }
}

/// A `$for` binder bound to one item of its iteration (spec 03, section 2): what that step
/// generates sees the binder as the item.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Staging {
    pub binder: Node<BinderId>,
    pub value: Value,
}

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

/// A function value with its `$` parameters bound to their values: what a body is typed and
/// lowered for (spec 02, section 3: generic code is checked per instantiation). A function
/// without `$` parameters, outside any call, has one instantiation.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Instantiation {
    pub f: Func,
    /// By parameter: the value of each `$` parameter, `None` for the others and for unknown ones.
    pub args: Vec<Option<Value>>,
}

impl Instantiation {
    /// The function literal `f`, outside any call, with no parameter bound.
    #[must_use]
    pub fn plain(f: ExprRef) -> Self {
        Self { f: Func { literal: f, within: None, staged: Arc::from([]) }, args: Vec::new() }
    }

    /// Whether this is its function literal outside any call, with no parameter bound.
    #[must_use]
    pub fn is_plain(&self) -> bool {
        *self == Self::plain(self.f.literal.clone())
    }

    /// The instantiation `call` runs: its function, with its `$` parameters bound to the call's
    /// arguments.
    #[must_use]
    pub fn of(cx: &Cx<'_>, call: &Instance) -> Option<Self> {
        let comptime = call.f.comptime(cx)?;
        let args = comptime.iter().zip(&call.args).map(|(&c, arg)| c.then(|| arg.clone()));
        Some(Self::new(call.f.clone(), args.collect()))
    }

    /// The environment the call `call` runs its body in: its parameters bound, its identity the
    /// call (decision 28), and the types typing found for the instantiation it runs.
    #[must_use]
    pub fn running<'a, 'db>(cx: &'a Cx<'db>, call: &Instance) -> Option<Env<'a, 'db>> {
        let types = cx.get::<crate::typeck::Typed>(&Self::of(cx, call)?);
        Some(Env::within(cx, call)?.typed_by(types))
    }

    /// `f` with its `$` parameters bound to `args`, by parameter: a function with none bound
    /// has one instantiation, however it is reached.
    #[must_use]
    pub fn new(f: Func, args: Vec<Option<Value>>) -> Self {
        let args = if args.iter().all(Option::is_none) { Vec::new() } else { args };
        Self { f, args }
    }

    /// The signature of this instantiation, its types evaluated with its `$` parameters bound.
    ///
    /// # Errors
    /// Returns why a parameter or result type has no value.
    pub fn signature(&self, cx: &Cx<'_>) -> Result<Signature, Unevaluated> {
        cx.get::<Signed>(self)
    }

    /// Whether this instantiation runs only at compile time: its result or a parameter passed at
    /// run time, not a `$` one, is of a type whose values do not exist at run time, such as
    /// `type`, code, an unbounded integer or a borrow of a plan.
    #[must_use]
    pub fn compile_time(&self, cx: &Cx<'_>) -> bool {
        let Ok(signature) = self.signature(cx) else { return false };
        let comptime = self.f.comptime(cx).unwrap_or_default();
        let runtime = signature.params.iter().enumerate().filter(|(i, _)| comptime.get(*i) != Some(&true));
        runtime.map(|(_, ty)| ty).chain([&signature.result]).any(|ty| !cx.get::<Exists>(ty))
    }
}

impl Func {}
