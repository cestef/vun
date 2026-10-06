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

impl Instance {
    /// This call as the instantiation binding every parameter to its argument.
    #[must_use]
    pub fn instantiation(&self) -> Instantiation {
        self.instantiation_of(self.args.len())
    }

    /// This call as the instantiation binding the first `params` parameters to their arguments:
    /// what follows them is storage lent to the call, which no parameter holds.
    #[must_use]
    pub fn instantiation_of(&self, params: usize) -> Instantiation {
        Instantiation::new(self.f.clone(), self.args.iter().take(params).cloned().map(Some).collect())
    }
}

/// A function value: a function literal, and the instantiation it was evaluated in, whose
/// parameters its body sees too: a method of `Pair(u8)` sees `T` bound to `u8`, and a closure in
/// a body with `$n: u8` sees `n`.
///
/// Shared, so that a value holding one is a pointer wide: a compile-time call keeps its arguments
/// and results in every frame it nests.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Func(Arc<FuncParts>);

#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct FuncParts {
    pub literal: ExprRef,
    pub within: Option<Arc<Instantiation>>,
    /// The `$for` bindings of the entries that generated it, which its body sees too.
    pub staged: Arc<[Staging]>,
}

impl Func {
    #[must_use]
    pub fn new(literal: ExprRef, within: Option<Arc<Instantiation>>, staged: Arc<[Staging]>) -> Self {
        Self(Arc::new(FuncParts { literal, within, staged }))
    }

    /// This function written at `literal` instead, in the same call.
    #[must_use]
    pub fn at(&self, literal: ExprRef) -> Self {
        Self::new(literal, self.within.clone(), self.staged.clone())
    }
}

impl std::ops::Deref for Func {
    type Target = FuncParts;

    fn deref(&self) -> &FuncParts {
        &self.0
    }
}

/// A code value (spec 03, section 4): an expression, and what it was written under, where its
/// names resolve (hygiene by construction): the instantiation and every binder bound there.
/// A quote `#{ e }` is one, and so is an argument for a `Code(T)` parameter. Shared, as a
/// function is.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Code(Arc<CodeParts>);

#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct CodeParts {
    pub expr: ExprRef,
    pub within: Option<Arc<Instantiation>>,
    pub bound: Arc<[Staging]>,
}

impl Code {
    #[must_use]
    pub fn new(expr: ExprRef, within: Option<Arc<Instantiation>>, bound: Arc<[Staging]>) -> Self {
        Self(Arc::new(CodeParts { expr, within, bound }))
    }
}

impl std::ops::Deref for Code {
    type Target = CodeParts;

    fn deref(&self) -> &CodeParts {
        &self.0
    }
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
    pub args: Arc<[Option<Value>]>,
}

impl Instantiation {
    /// The function literal `f`, outside any call, with no parameter bound.
    #[must_use]
    pub fn plain(f: ExprRef) -> Self {
        Self { f: Func::new(f, None, Arc::from([])), args: Arc::from([]) }
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
        Self { f, args: args.into() }
    }

    /// The compile-time call this is, when it binds every parameter: `Pair(u8)` for a type made
    /// by `Pair`.
    #[must_use]
    pub fn call(&self) -> Option<Instance> {
        Some(Instance { f: self.f.clone(), args: self.args.iter().cloned().collect::<Option<_>>()? })
    }
}
