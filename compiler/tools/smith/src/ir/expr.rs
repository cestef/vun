//! The forms of expressions.

use vun_grammar::TokenKind;
use vun_items::Item;
use vun_items::std::int_Narrow as Method;

use super::{FnId, Label, LocalId, Place, Stmt, TypeId};
use crate::ty::Nominal;
use crate::ty::Ty;
use crate::value::Value;

/// An expression and its type, which no form is made without.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Expr {
    pub ty: Ty,
    pub kind: Kind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Lit(Value),
    Local(LocalId),
    Unary(Unary, Box<Expr>),
    Arith(Arith, Box<Expr>, Box<Expr>),
    Compare(Compare, Box<Expr>, Box<Expr>),
    Logic(Logic, Box<Expr>, Box<Expr>),
    Narrow(Narrow, Box<Expr>),
    If(Box<Expr>, Box<Expr>, Box<Expr>),
    Tuple(Vec<Expr>),
    Array(Vec<Expr>),
    Repeat(Box<Expr>, usize),
    Field(Box<Expr>, usize),
    Index(Box<Expr>, Box<Expr>),
    /// A call of a function, given the types for its `$T: type` parameters first.
    Call(FnId, Vec<Ty>, Vec<Expr>),
    Block(Vec<Stmt>, Box<Expr>),
    /// `'label: for { body }`, which only a `break` with a value leaves; the value is the loop's.
    Forever(Label, Vec<Stmt>),
    /// A value of a struct, its fields in order.
    Struct(TypeId, Vec<Expr>),
    /// A value of an enum: the variant, and its payload.
    Variant(Nominal, usize, Vec<Expr>),
    /// The first arm whose pattern the value matches.
    Match(Box<Expr>, Vec<Arm>),
    /// A closure written: it captures, and is called as any value of its type.
    Closure(Box<Closure>),
    /// `callee(args)`, of a closure held in a local.
    CallClosure(Box<Expr>, Vec<Expr>),
    /// `&place` or `&mut place`
    Borrow(bool, Place),
    /// `reference.*`
    Deref(Box<Expr>),
    /// `x.nearest(T)` or `x.truncate(T)`, to the type of the expression.
    Convert(Convert, Box<Expr>),
    /// `x.try_to(T)`: an option of `T`, holding `x` if `T` does.
    TryTo(Box<Expr>),
    /// `o.unwrap_or(default)`
    UnwrapOr(Box<Expr>, Box<Expr>),
    /// `o.is_some()`
    IsSome(Box<Expr>),
    /// `lang.flow.tail(function, args)` in return position: the value of the call, made by a jump.
    TailCall(FnId, Vec<Expr>),
    /// `e?`: the payload of an option or a result, or the function returns what is not one.
    Try(Box<Expr>),
}

/// `[captures] (params) -> result { body }`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Closure {
    /// Which closure of the program, which the types of its values say.
    pub id: usize,
    /// What the body reads and writes of the function around it.
    pub captures: Vec<Capture>,
    /// The locals the arguments are bound to.
    pub params: Vec<LocalId>,
    /// The result as written; the type of the body where none is.
    pub result: Option<Ty>,
    pub body: Expr,
}

/// A local of the function around a closure, and how the closure holds it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capture {
    pub local: LocalId,
    pub how: Capturing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, vun_variants::Variants)]
pub enum Capturing {
    /// A copy of the value, taken where the closure is made.
    Copy,
    /// `&local`
    Shared,
    /// `&mut local`
    Mut,
}

/// `pattern => body`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Arm {
    pub pattern: Pattern,
    pub body: Expr,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pattern {
    /// A variant, with a local bound to each part of its payload that is used, or `None` to
    /// discard it.
    Variant { variant: usize, binds: Vec<Option<LocalId>> },
    /// `_`
    Any,
}

impl Expr {
    /// The expressions directly inside it, a block's tail but not its statements.
    pub fn children_mut(&mut self) -> Vec<&mut Self> {
        match &mut self.kind {
            Kind::Lit(_) | Kind::Local(_) | Kind::Forever(..) => Vec::new(),
            Kind::Borrow(_, place) => place.indices_mut().collect(),
            Kind::Closure(closure) => vec![&mut closure.body],
            Kind::CallClosure(callee, args) => {
                std::iter::once(&mut **callee).chain(args.iter_mut()).collect()
            }
            Kind::Unary(_, a)
            | Kind::Deref(a)
            | Kind::Convert(_, a)
            | Kind::Narrow(_, a)
            | Kind::TryTo(a)
            | Kind::Try(a)
            | Kind::IsSome(a)
            | Kind::Repeat(a, _)
            | Kind::Field(a, _) => vec![&mut **a],
            Kind::UnwrapOr(a, b)
            | Kind::Arith(_, a, b)
            | Kind::Compare(_, a, b)
            | Kind::Logic(_, a, b)
            | Kind::Index(a, b) => {
                vec![&mut **a, &mut **b]
            }
            Kind::If(c, a, b) => vec![&mut **c, &mut **a, &mut **b],
            Kind::Tuple(items)
            | Kind::Array(items)
            | Kind::Call(_, _, items)
            | Kind::TailCall(_, items)
            | Kind::Struct(_, items)
            | Kind::Variant(_, _, items) => items.iter_mut().collect(),
            Kind::Block(_, tail) => vec![&mut **tail],
            Kind::Match(scrutinee, arms) => {
                std::iter::once(&mut **scrutinee).chain(arms.iter_mut().map(|a| &mut a.body)).collect()
            }
        }
    }
}

/// The conversions of floats (spec 04, section 4): all explicit, all total.
#[derive(Clone, Copy, Debug, PartialEq, Eq, vun_variants::Variants)]
pub enum Convert {
    /// The float nearest an integer or another float.
    Nearest,
    /// The integer a float rounds toward zero to, clamped.
    Truncate,
}

impl Convert {
    /// The method of the operand that does it.
    #[must_use]
    pub const fn method(self) -> &'static str {
        match self {
            Self::Nearest => "nearest",
            Self::Truncate => "truncate",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, vun_variants::Variants)]
pub enum Unary {
    /// `-x`, on an integer.
    Neg,
    /// `~x`, on a `uN`.
    BitNot,
    /// `!x`, on a bool.
    Not,
}

impl Unary {
    /// The operator's token.
    #[must_use]
    pub const fn token(self) -> TokenKind {
        match self {
            Self::Neg => TokenKind::Minus,
            Self::BitNot => TokenKind::Tilde,
            Self::Not => TokenKind::Bang,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, vun_variants::Variants)]
pub enum Arith {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    And,
    Or,
    Xor,
    Shl,
    Shr,
}

impl Arith {
    /// The operator's token.
    #[must_use]
    pub const fn token(self) -> TokenKind {
        match self {
            Self::Add => TokenKind::Plus,
            Self::Sub => TokenKind::Minus,
            Self::Mul => TokenKind::Star,
            Self::Div => TokenKind::Slash,
            Self::Rem => TokenKind::Percent,
            Self::And => TokenKind::Amp,
            Self::Or => TokenKind::Pipe,
            Self::Xor => TokenKind::Caret,
            Self::Shl => TokenKind::Shl,
            Self::Shr => TokenKind::Shr,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, vun_variants::Variants)]
pub enum Compare {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl Compare {
    /// The operator's token.
    #[must_use]
    pub const fn token(self) -> TokenKind {
        match self {
            Self::Eq => TokenKind::EqEq,
            Self::Ne => TokenKind::BangEq,
            Self::Lt => TokenKind::Lt,
            Self::Le => TokenKind::Le,
            Self::Gt => TokenKind::Gt,
            Self::Ge => TokenKind::Ge,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, vun_variants::Variants)]
pub enum Logic {
    And,
    Or,
}

impl Logic {
    /// The operator's token.
    #[must_use]
    pub const fn token(self) -> TokenKind {
        match self {
            Self::And => TokenKind::And,
            Self::Or => TokenKind::Or,
        }
    }
}

/// How an integer becomes another type (spec 04, rule 3), by the methods that never trap.
#[derive(Clone, Copy, Debug, PartialEq, Eq, vun_variants::Variants)]
pub enum Narrow {
    Wrap,
    Sat,
    /// Traps outside the type; made only of a value the type holds.
    To,
}

impl Narrow {
    /// The method of `std.int.Narrow` that does it.
    #[must_use]
    pub const fn method(self) -> Item {
        match self {
            Self::Wrap => Method::wrap,
            Self::Sat => Method::sat,
            Self::To => Method::to,
        }
    }
}
