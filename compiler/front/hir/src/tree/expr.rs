//! Expressions, including types, which are compile-time values (spec 02).

use vun_grammar::TokenKind;
use vun_intern::Symbol;
use vun_origin::OriginId;

use super::{Arm, BlockId, ExprId, List, Name, NsId, Param, PatId, Variant};

pub struct Expr {
    pub kind: ExprKind,
    pub origin: OriginId,
}

pub enum Lit {
    Int(i128),
    Float(String),
    Char(char),
    String(String),
}

/// An argument or a literal field: `value`, or `.name = value`.
pub struct Arg {
    pub name: Option<Name>,
    pub value: ExprId,
}

/// The head of an `if`: a condition, or a pattern test `p = e`.
pub enum Cond {
    Expr(ExprId),
    Test { pat: PatId, value: ExprId },
}

pub enum ForHead {
    Forever,
    While(Cond),
    In { pat: PatId, iter: ExprId },
}

pub enum ExprKind {
    Error,
    /// `_` as a value: provided from outside (spec 01, section 3).
    Hole,
    Name(Symbol),
    Lit(Lit),
    /// `.name`, looked up in the expected type.
    DotName(Name),
    Member {
        object: ExprId,
        name: Name,
    },
    Call {
        callee: ExprId,
        args: List<Arg>,
    },
    Index {
        object: ExprId,
        index: ExprId,
    },
    /// `.{ }` without a type, `T{ }` with one.
    StructLit {
        ty: Option<ExprId>,
        fields: List<Arg>,
    },
    Tuple(List<ExprId>),
    Array(List<ExprId>),
    Repeat {
        value: ExprId,
        count: ExprId,
    },
    ArrayType {
        len: ExprId,
        elem: ExprId,
    },
    /// `op operand`; after `&`, `mode` is the borrow mode's words (`mut`, `out`, `temp mut`, ...).
    Prefix {
        op: TokenKind,
        mode: Option<Symbol>,
        operand: ExprId,
    },
    Postfix {
        op: TokenKind,
        operand: ExprId,
    },
    Infix {
        op: TokenKind,
        left: ExprId,
        right: Option<ExprId>,
    },
    Block(BlockId),
    Quote(BlockId),
    If {
        cond: Cond,
        then: BlockId,
        otherwise: Option<ExprId>,
    },
    For {
        head: ForHead,
        body: BlockId,
    },
    Match {
        scrutinee: ExprId,
        arms: List<Arm>,
    },
    Labeled {
        label: Symbol,
        body: ExprId,
    },
    Return(Option<ExprId>),
    Break {
        label: Option<Symbol>,
        value: Option<ExprId>,
    },
    Continue {
        label: Option<Symbol>,
    },
    Yield(ExprId),
    Defer(ExprId),
    Fn {
        params: List<Param>,
        returns: Option<ExprId>,
        body: BlockId,
    },
    FnType {
        params: List<Param>,
        returns: Option<ExprId>,
    },
    Closure {
        captures: List<ExprId>,
        func: ExprId,
    },
    Struct(NsId),
    /// `enum { variants; declarations }`: the declarations are its namespace, as a struct's.
    Enum {
        variants: List<Variant>,
        ns: NsId,
    },
    /// `#[attrs] e` on an expression, such as a function type with an ABI.
    Attributed {
        attrs: Vec<ExprId>,
        inner: ExprId,
    },
}

impl ExprKind {
    /// The name of this kind of expression.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            ExprKind::Error => "Error",
            ExprKind::Hole => "Hole",
            ExprKind::Name(_) => "Name",
            ExprKind::Lit(_) => "Lit",
            ExprKind::DotName(_) => "DotName",
            ExprKind::Member { .. } => "Member",
            ExprKind::Call { .. } => "Call",
            ExprKind::Index { .. } => "Index",
            ExprKind::StructLit { .. } => "StructLit",
            ExprKind::Tuple(_) => "Tuple",
            ExprKind::Array(_) => "Array",
            ExprKind::Repeat { .. } => "Repeat",
            ExprKind::ArrayType { .. } => "ArrayType",
            ExprKind::Prefix { .. } => "Prefix",
            ExprKind::Postfix { .. } => "Postfix",
            ExprKind::Infix { .. } => "Infix",
            ExprKind::Block(_) => "Block",
            ExprKind::Quote(_) => "Quote",
            ExprKind::If { .. } => "If",
            ExprKind::For { .. } => "For",
            ExprKind::Match { .. } => "Match",
            ExprKind::Labeled { .. } => "Labeled",
            ExprKind::Return(_) => "Return",
            ExprKind::Break { .. } => "Break",
            ExprKind::Continue { .. } => "Continue",
            ExprKind::Yield(_) => "Yield",
            ExprKind::Defer(_) => "Defer",
            ExprKind::Fn { .. } => "Fn",
            ExprKind::FnType { .. } => "FnType",
            ExprKind::Closure { .. } => "Closure",
            ExprKind::Struct(_) => "Struct",
            ExprKind::Enum { .. } => "Enum",
            ExprKind::Attributed { .. } => "Attributed",
        }
    }
}
