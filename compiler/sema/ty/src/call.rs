//! Compile-time calls as values: a function value, a call of one, its instantiation, a code
//! value and a suspended coroutine frame.

use std::sync::Arc;

use vun_codec::{Decode, Encode};
use vun_desugar::ITERATION;
use vun_hir::tree::BinderId;

use crate::item::{ExprRef, Node};
use crate::{Ty, Value};

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

    /// `f` with its `$` parameters bound to `args`, by parameter: a function with none bound
    /// has one instantiation, however it is reached.
    #[must_use]
    pub fn new(f: Func, args: Vec<Option<Value>>) -> Self {
        let args = if args.iter().all(Option::is_none) { Vec::new() } else { args };
        Self { f, args }
    }
}
