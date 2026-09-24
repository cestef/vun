//! Function bodies: locals, and basic blocks of statements ending in a terminator (spec 13,
//! section 3). Every local, statement and terminator carries the origin that produced it.

use overt_grammar::TokenKind;
use overt_hir::tree::ExprId;
use overt_index::{IndexVec, index};
use overt_intern::Symbol;
use overt_origin::OriginId;
use overt_sema::ty::{NominalKind, Value};

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

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LocalDecl {
    /// `None` for a temporary.
    pub name: Option<Symbol>,
    pub origin: OriginId,
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
    Call {
        callee: Operand,
        args: Vec<Operand>,
    },
    /// The type a `struct` or `enum` literal of the body's file evaluates to, identified by the
    /// literal and the call running the body (decision 28).
    Nominal {
        literal: ExprId,
        kind: NominalKind,
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

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TerminatorKind {
    Return,
    Goto(BlockId),
}
