//! What a statement computes: the operands it reads and the value it builds from them.

use vun_codec::{Decode, Encode};
use vun_desugar::{Borrow, Narrowing};
use vun_grammar::TokenKind;
use vun_hir::tree::ExprId;
use vun_intern::Symbol;
use vun_sema::env::{Env, Unevaluated};
use vun_sema::eval::{Code, Staging};
use vun_sema::ty::{Ty, Value};

use crate::place::Place;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Rvalue {
    Use(Operand),
    /// `!c`: the other condition (the table `forms`). Operators are calls of the functions of
    /// their `lang.ops` interfaces (spec 02, section 4).
    Not {
        operand: Operand,
    },
    /// `lo..hi`, `lo..=hi` or `lo..`, by its operator.
    Range {
        op: TokenKind,
        lo: Operand,
        hi: Option<Operand>,
    },
    /// A value built from parts, in the order `kind` gives.
    Aggregate {
        kind: Aggregate,
        parts: Vec<Operand>,
    },
    /// `&mode place`: a borrow of the place in the mode `mode` (the table `borrows`).
    Ref {
        mode: Borrow,
        place: Place,
    },
    /// Resumes the frame at `frame` until it yields or returns, giving a value of the type
    /// `step` its interface's `resume` gives (spec 06).
    Resume {
        frame: Place,
        step: Ty,
    },
    /// Ends the frame at `frame`, consuming it: a finished frame is freed, a suspended one runs
    /// the defers pending at its suspension point (spec 06, section 1).
    End {
        frame: Place,
    },
    /// The binding of the place assigned goes out of scope: its storage ends, and a linear value
    /// still held there is leaked (spec 05, section 2). Written at every exit of the binding's
    /// block, after the block's deferred statements.
    Dead,
    /// `[operand; count]`: an array of `count` copies, `count` known at compile time.
    Repeat {
        operand: Operand,
        count: Value,
    },
    /// Whether the value at `place` is the variant `variant`: a condition (the table `forms`).
    IsVariant {
        place: Place,
        variant: Symbol,
    },
    Call {
        callee: Operand,
        args: Vec<Operand>,
    },
    /// `operand` brought into the integer type `ty` as a compound assignment does (spec 04,
    /// rule 9): a trap site for [`Narrowing::To`].
    Narrow {
        operand: Operand,
        ty: Ty,
        narrowing: Narrowing,
    },
    /// A compile-time value that depends on the call running the body, evaluated in its
    /// environment: a `struct` or `enum` literal, identified by the call (decision 28), a
    /// function literal, which sees the call's parameters, or such a parameter of an enclosing
    /// function. `staged` are the `$for` bindings of the step it is written in (spec 03,
    /// section 1).
    Comptime {
        expr: ExprId,
        staged: Vec<Staging>,
        /// The type a value is wanted of, which forms typed by their context read: the type of
        /// the `$` parameter the value is passed to.
        expected: Option<Ty>,
    },
    /// The code `expr`, written in the call running the body under the bindings `staged`: a
    /// quote, or an argument for a `Code(T)` parameter (spec 03, section 4).
    Quote {
        expr: ExprId,
        staged: Vec<Staging>,
    },
    /// `$c` at run time, for code `c` that reads what only its call site holds: the value of the
    /// code, built where it is written by the call site expanding this body (spec 03, section 4).
    Splice {
        code: Code,
    },
}

impl Rvalue {
    /// The operands this reads, in order, to rewrite.
    pub fn operands_mut(&mut self) -> Vec<&mut Operand> {
        match self {
            Self::Use(o)
            | Self::Not { operand: o }
            | Self::Narrow { operand: o, .. }
            | Self::Repeat { operand: o, .. } => {
                vec![o]
            }
            Self::Range { lo, hi, .. } => std::iter::once(lo).chain(hi).collect(),
            Self::Aggregate { parts, .. } => parts.iter_mut().collect(),
            Self::Call { callee, args } => std::iter::once(callee).chain(args).collect(),
            Self::Ref { .. }
            | Self::Resume { .. }
            | Self::End { .. }
            | Self::IsVariant { .. }
            | Self::Comptime { .. }
            | Self::Quote { .. }
            | Self::Splice { .. }
            | Self::Dead => Vec::new(),
        }
    }

    /// The operands this reads, in order.
    #[must_use]
    pub fn operands(&self) -> Vec<&Operand> {
        match self {
            Self::Use(o)
            | Self::Not { operand: o }
            | Self::Narrow { operand: o, .. }
            | Self::Repeat { operand: o, .. } => {
                vec![o]
            }
            Self::Range { lo, hi, .. } => std::iter::once(lo).chain(hi).collect(),
            Self::Aggregate { parts, .. } => parts.iter().collect(),
            Self::Call { callee, args } => std::iter::once(callee).chain(args).collect(),
            Self::Ref { .. }
            | Self::Resume { .. }
            | Self::End { .. }
            | Self::IsVariant { .. }
            | Self::Comptime { .. }
            | Self::Quote { .. }
            | Self::Splice { .. }
            | Self::Dead => Vec::new(),
        }
    }
}

impl Rvalue {
    /// The value of a compile-time rvalue, `Comptime` or `Quote`, in `env`, the environment of
    /// the call running the body; `None` for any other.
    #[must_use]
    pub fn compile_time(&self, env: &Env<'_, '_>) -> Option<Result<Value, Unevaluated>> {
        match self {
            Self::Comptime { expr, staged, expected } => {
                Some(env.clone().staging(staged).eval_as(*expr, *expected))
            }
            Self::Quote { expr, staged } => Some(Ok(Value::Code(env.clone().staging(staged).code(*expr)))),
            _ => None,
        }
    }
}

/// What an [`Rvalue::Aggregate`] builds.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Aggregate {
    /// A value of the struct type `ty`: its fields `fields`, in declaration order.
    Struct {
        ty: Ty,
        fields: Vec<Symbol>,
    },
    /// The variant `name` of the enum type `ty`, its parts the payload.
    Variant {
        ty: Ty,
        name: Symbol,
    },
    Tuple,
    Array,
    /// A closure: the function literal `literal`, evaluated in the call running the body under
    /// the bindings `staged`, and its captures, the parts in the order written.
    Closure {
        literal: ExprId,
        staged: Vec<Staging>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Operand {
    Place(Place),
    Const(Value),
}
