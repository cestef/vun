//! The interface to compile-time evaluation (spec 13, section 6).
//!
//! Sema defines what it needs from an evaluator; `vun-interp` implements it, and the driver
//! installs it in the query database, so that sema calls it without depending on it. Calls are
//! memoized on the function and its arguments (spec 03, section 7), which also makes `Pair(u16)`
//! one type.

use std::sync::{Arc, OnceLock};

use vun_desugar::ITERATION;
use vun_hir::db::Parsed;
use vun_hir::tree::{BinderId, Entry, ExprKind, Param, PatKind};
use vun_intern::Symbol;
use vun_query::{Cx, Cycle, Query};

use crate::env::{Env, Unevaluated};
use crate::item::{ExprRef, Node};
use crate::trap::Reason;
use crate::ty::{Signature, Ty, Value};

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
    /// The `$for` bindings of the entries that generated it, which its body sees too.
    pub staged: Arc<[Staging]>,
}

/// A code value (spec 03, section 4): an expression, and what it was written under, where its
/// names resolve (hygiene by construction): the compile-time call and every binder bound there.
/// A quote `#{ e }` is one, and so is an argument for a `Code(T)` parameter.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Code {
    pub expr: ExprRef,
    pub within: Option<Arc<Instance>>,
    pub bound: Arc<[Staging]>,
}

/// A coroutine's frame at compile time (spec 06): its call, the block of its body it resumes at
/// (`None` once it returned), and its locals.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Suspended {
    pub call: Instance,
    pub at: Option<usize>,
    pub locals: Vec<Option<Value>>,
}

/// What resuming a frame gives: an item it yields, or what it returns.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Resumed {
    Yielded(Value),
    Returned(Value),
}

impl Resumed {
    /// This outcome as a value of `step`, the type resuming gives: its variant `item` or `done`
    /// (the table `iteration`).
    #[must_use]
    pub fn value(self, step: Ty) -> Option<Value> {
        let &(.., item, done) = ITERATION.first()?;
        let (name, value) = match self {
            Self::Yielded(value) => (item, value),
            Self::Returned(value) => (done, value),
        };
        Some(Value::Variant { ty: step, name: Symbol::intern(name), payload: vec![value] })
    }
}

/// A `$for` binder bound to one item of its iteration (spec 03, section 2): what that step
/// generates sees the binder as the item.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
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
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
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

/// A compile-time call, memoized on its function and arguments, with the steps it took.
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
            Some(evaluator) => evaluator.call(cx, call),
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
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
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

    /// The instantiation `call` runs: its function, with its `$` parameters bound to the call's
    /// arguments.
    #[must_use]
    pub fn of(cx: &Cx<'_>, call: &Instance) -> Option<Self> {
        let comptime = call.f.comptime(cx)?;
        let args = comptime.iter().zip(&call.args).map(|(&c, arg)| c.then(|| arg.clone())).collect();
        Some(Self { f: call.f.clone(), args })
    }

    /// The signature of this instantiation, its types evaluated with its `$` parameters bound.
    ///
    /// # Errors
    /// Returns why a parameter or result type has no value.
    pub fn signature(&self, cx: &Cx<'_>) -> Result<Signature, Unevaluated> {
        let inside = Env::inside(cx, &self.f, self.args.iter().cloned()).ok_or(Unevaluated::Absent)?;
        inside.literal_signature(self.f.literal.expr.id, &self.args)
    }
}

impl Func {
    /// Whether each parameter of this function is a `$` parameter; `None` when its file is not
    /// read.
    #[must_use]
    pub fn comptime(&self, cx: &Cx<'_>) -> Option<Vec<bool>> {
        let parsed = cx.get::<Parsed>(&self.literal.file)?;
        let hir = &parsed.hir;
        let ExprKind::Fn { params, .. } = &hir.exprs[self.literal.expr.id].kind else { return None };
        let comptime = params.iter().map(|p| match p {
            Entry::One(Param { pat: Some(pat), .. }) => {
                matches!(hir.pats[*pat].kind, PatKind::Bind(b) if hir.binders[b].comptime)
            }
            _ => false,
        });
        Some(comptime.collect())
    }
}
