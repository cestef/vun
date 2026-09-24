//! Function bodies: locals, and basic blocks of statements ending in a terminator (spec 13,
//! section 3). Every local, statement and terminator carries the origin that produced it.

use vun_grammar::{Narrowing, TokenKind};
use vun_hir::tree::ExprId;
use vun_index::{IndexVec, index};
use vun_intern::Symbol;
use vun_origin::OriginId;
use vun_sema::eval::Staging;
use vun_sema::ty::{Ty, Value};

use crate::place::Place;

index! {
    pub struct Local;
    pub struct BlockId;
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Body {
    pub locals: IndexVec<Local, LocalDecl>,
    /// The first locals, in order.
    pub params: Vec<Local>,
    /// Where the result is written before returning.
    pub ret: Local,
    pub blocks: IndexVec<BlockId, Block>,
    pub origin: OriginId,
}

impl Body {
    /// Whether this is a coroutine function's body: one that yields.
    #[must_use]
    pub fn suspends(&self) -> bool {
        self.blocks.iter().any(|(_, b)| matches!(b.term.kind, TerminatorKind::Yield { .. }))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LocalDecl {
    /// `None` for a temporary.
    pub name: Option<Symbol>,
    /// Its type, where known.
    pub ty: Option<Ty>,
    pub role: Role,
    pub origin: OriginId,
}

/// What a local holds for the checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    /// A value, which is consumed as a whole.
    Held,
    /// A value a pattern takes apart: its parts, bound or dropped, carry what it held.
    Unpacked,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub term: Terminator,
}

/// `place = value`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Stmt {
    pub place: Place,
    pub value: Rvalue,
    pub origin: OriginId,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Rvalue {
    Use(Operand),
    /// An operator of the grammar's infix table, on integers: the `lang.ops` impls the compiler
    /// provides.
    Binary {
        op: TokenKind,
        left: Operand,
        right: Operand,
    },
    Unary {
        op: TokenKind,
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
    /// `&mode place`: a borrow of the place in the mode the word after `&` names (the table
    /// `borrows`), `None` for a shared one.
    Ref {
        mode: Option<Symbol>,
        place: Place,
    },
    /// Resumes the frame at `frame` until it yields or returns, giving a value of the type
    /// `step` its interface's `resume` gives (spec 06).
    Resume {
        frame: Place,
        step: Ty,
    },
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
    },
    /// The code `expr`, written in the call running the body under the bindings `staged`: a
    /// quote, or an argument for a `Code(T)` parameter (spec 03, section 4).
    Quote {
        expr: ExprId,
        staged: Vec<Staging>,
    },
}

impl Rvalue {
    /// The operands this reads, in order.
    #[must_use]
    pub fn operands(&self) -> Vec<&Operand> {
        match self {
            Self::Use(o)
            | Self::Unary { operand: o, .. }
            | Self::Narrow { operand: o, .. }
            | Self::Repeat { operand: o, .. } => {
                vec![o]
            }
            Self::Binary { left, right, .. } => vec![left, right],
            Self::Range { lo, hi, .. } => std::iter::once(lo).chain(hi).collect(),
            Self::Aggregate { parts, .. } => parts.iter().collect(),
            Self::Call { callee, args } => std::iter::once(callee).chain(args).collect(),
            Self::Ref { .. }
            | Self::Resume { .. }
            | Self::IsVariant { .. }
            | Self::Comptime { .. }
            | Self::Quote { .. } => Vec::new(),
        }
    }
}

/// What an [`Rvalue::Aggregate`] builds.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
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

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Operand {
    Place(Place),
    Const(Value),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Terminator {
    pub kind: TerminatorKind,
    pub origin: OriginId,
}

impl TerminatorKind {
    /// The blocks control may go to next.
    #[must_use]
    pub fn successors(&self) -> Vec<BlockId> {
        match self {
            Self::Return | Self::Unreachable => Vec::new(),
            Self::Goto(to) => vec![*to],
            Self::Yield { resume, .. } => vec![*resume],
            Self::Branch { then, otherwise, .. } => vec![*then, *otherwise],
            Self::Defer { run, skip, .. } => vec![*run, *skip],
        }
    }

    /// The operands this reads.
    #[must_use]
    pub fn operands(&self) -> Vec<&Operand> {
        match self {
            Self::Branch { cond: o, .. } | Self::Yield { value: o, .. } => vec![o],
            _ => Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TerminatorKind {
    Return,
    /// Control never gets here, such as past the arms of an exhaustive `match`.
    Unreachable,
    /// `yield value`: suspends the coroutine with `value`; resuming it goes on at `resume`.
    Yield {
        value: Operand,
        resume: BlockId,
    },
    Goto(BlockId),
    /// To `then` when the condition `cond` holds, to `otherwise` when it does not.
    Branch {
        cond: Operand,
        then: BlockId,
        otherwise: BlockId,
    },
    /// A deferred statement at an exit (spec 05, section 3): to `run`, which lowers it, when
    /// every linear local it consumes is `init` here, to `skip` when they were moved. The
    /// place states decide which, statically.
    Defer {
        consumes: Vec<Local>,
        run: BlockId,
        skip: BlockId,
    },
}
