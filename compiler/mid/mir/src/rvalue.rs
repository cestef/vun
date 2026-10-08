//! What a statement computes: the operands it reads and the value it builds from them.

use vun_codec::{Decode, Encode};
use vun_desugar::{Borrow, Narrowing};
use vun_grammar::TokenKind;
use vun_hir::tree::ExprId;
use vun_intern::Symbol;
use vun_primitives::Primitive;
use vun_primitives::flow::Flow;
use vun_sema::env::Env;
use vun_sema::typed::Step;
use vun_sema_ty::call::{Code, Staging};
use vun_sema_ty::trap::Unevaluated;
use vun_sema_ty::{Ty, Value};

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
    /// function, in the step `at` (spec 03, section 1).
    Comptime {
        expr: ExprId,
        at: Stepped,
        /// The type a value is wanted of, which forms typed by their context read: the type of
        /// the `$` parameter the value is passed to.
        expected: Option<Ty>,
    },
    /// The code `expr`, written in the call running the body in the step `at`: a quote, or an
    /// argument for a `Code(T)` parameter (spec 03, section 4).
    Quote {
        expr: ExprId,
        at: Stepped,
    },
    /// `$c` at run time, for code `c` that reads what only its call site holds: the value of the
    /// code, built where it is written by the call site expanding this body (spec 03, section 4).
    Splice {
        code: Code,
    },
    /// An inline assembly block (spec 07, section 6): it reads its inputs and the declarations
    /// whose addresses it names, writes its outputs, and does anything to memory and to the
    /// registers it names.
    Asm(Box<Asm>),
}

/// An inline assembly block as MIR holds it: inputs as operands, outputs as places, a
/// declaration's address as the operand naming the declaration.
pub type Asm = vun_inline::Block<Operand, Place, Operand>;

impl Rvalue {
    /// Whether this is `lang.flow.tail(f, args)`, which replaces the frame of the function running
    /// it by a call (spec 09, section 3).
    #[must_use]
    pub fn is_tail_call(&self) -> bool {
        matches!(
            self,
            Self::Call { callee: Operand::Const(Value::Primitive(Primitive::Flow(Flow::tail))), .. }
        )
    }

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
            Self::Asm(asm) => asm.reads_mut().map(vun_inline::Read::read).collect(),
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

    /// The operands a constant may replace: all of them, but the values a block binds, which stay
    /// in the locals typing gave their type, as an instruction takes a value at that type and a
    /// constant's own is the narrowest its value fits.
    pub fn constant_operands_mut(&mut self) -> Vec<&mut Operand> {
        match self {
            Self::Asm(asm) => asm
                .reads_mut()
                .filter_map(|read| match read {
                    vun_inline::Read::Symbol(symbol) => Some(symbol),
                    vun_inline::Read::Value(_) => None,
                })
                .collect(),
            _ => self.operands_mut(),
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
            Self::Asm(asm) => asm.reads().map(vun_inline::Read::read).collect(),
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

    /// The places this writes besides the statement's own: an assembly block's outputs, each
    /// with whether it reads it first.
    #[must_use]
    pub fn outputs(&self) -> Vec<(&Place, bool)> {
        match self {
            Self::Asm(asm) => asm.outputs().collect(),
            _ => Vec::new(),
        }
    }

    /// The places `outputs` names, to renumber.
    pub fn outputs_mut(&mut self) -> Vec<&mut Place> {
        match self {
            Self::Asm(asm) => asm.outputs_mut().collect(),
            _ => Vec::new(),
        }
    }
}

impl Rvalue {
    /// The value of a compile-time rvalue, `Comptime` or `Quote`, in `env`, the environment of
    /// the call running the body; `None` for any other.
    #[must_use]
    pub fn compile_time(&self, env: &Env<'_, '_>) -> Option<Result<Value, Unevaluated>> {
        match self {
            Self::Comptime { expr, at, expected } => Some(at.env(env).eval_as(*expr, *expected)),
            Self::Quote { expr, at } => Some(Ok(Value::Code(at.env(env).code(*expr)))),
            _ => None,
        }
    }
}

/// Where in a body a compile-time rvalue is written: in the `$for` steps and tested `$if`
/// branches `path`, each inside the one before, under their bindings `staged`. Each step's forms
/// have the types typing found for that step apart, which evaluating them reads.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Stepped {
    pub staged: Vec<Staging>,
    pub path: Vec<Step>,
}

impl Stepped {
    /// `env`, the environment of the call running the body, in these steps.
    #[must_use]
    pub fn env<'a, 'db>(&self, env: &Env<'a, 'db>) -> Env<'a, 'db> {
        env.clone().staging(&self.staged).stepping(&self.path)
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
