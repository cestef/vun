//! What an expression holds besides expressions: blocks, fields, parameters, arms.

use vun_grammar::{BorrowWord, TokenKind};

use super::{Expr, Item};

/// A statement list: what is run in order, then the value of the block.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Block {
    pub stmts: Vec<Expr>,
    pub tail: Option<Box<Expr>>,
}

/// `name: ty = default`, a field of a struct type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    pub ty: Expr,
    pub default: Option<Expr>,
}

/// `pattern: ty`, a parameter of a function; of a function type the pattern may be left out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Param {
    pub pattern: Option<Expr>,
    pub ty: Expr,
}

/// An entry of `.{ }` or `T{ }`: a value, or a field by name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Init {
    Item(Expr),
    Named(String, Expr),
}

/// What a struct or an enum holds besides its variants.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Member {
    Field(Field),
    Item(Item),
}

/// `name` or `name(payload)`, a variant of an enum.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Variant {
    pub name: String,
    pub payload: Vec<Expr>,
}

/// `pattern if guard => value`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Arm {
    pub pattern: Expr,
    pub guard: Option<Expr>,
    pub value: Option<Expr>,
}

/// A function literal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Function {
    pub params: Vec<Param>,
    pub result: Option<Box<Expr>>,
    pub body: Block,
}

/// What follows the dot of a member access.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Property {
    Name(String),
    Index(u32),
}

/// The words after `&`: how long a call holds the borrow, and what it does to the place.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Borrow {
    pub qualifier: Option<BorrowWord>,
    pub mode: Option<BorrowWord>,
}

impl Borrow {
    /// `&mut`
    pub const MUT: Self = Self { qualifier: None, mode: Some(BorrowWord::Mut) };
    /// `&`
    pub const SHARED: Self = Self { qualifier: None, mode: None };
}

impl Field {
    /// `name: ty`
    #[must_use]
    pub fn new(name: &str, ty: Expr) -> Self {
        Self { name: name.to_owned(), ty, default: None }
    }
}

impl Param {
    /// `name: ty`
    #[must_use]
    pub fn named(name: &str, ty: Expr) -> Self {
        Self { pattern: Some(Expr::local(name)), ty }
    }

    /// `$name: ty`, a parameter known when compiling.
    #[must_use]
    pub fn comptime(name: &str, ty: Expr) -> Self {
        Self { pattern: Some(Expr::Staged(Box::new(Expr::local(name)))), ty }
    }

    /// `..name: ty`, the parameter gathering the arguments after the others.
    #[must_use]
    pub fn pack(name: &str, ty: Expr) -> Self {
        Self { pattern: Some(Expr::prefix(TokenKind::DotDot, Expr::local(name))), ty }
    }

    /// `ty`, in a function type.
    #[must_use]
    pub fn unnamed(ty: Expr) -> Self {
        Self { pattern: None, ty }
    }
}

impl Variant {
    /// `name(payload)`
    #[must_use]
    pub fn new(name: &str, payload: Vec<Expr>) -> Self {
        Self { name: name.to_owned(), payload }
    }
}

impl Block {
    /// `{ stmts; tail }`
    #[must_use]
    pub fn new(stmts: Vec<Expr>, tail: Option<Expr>) -> Self {
        Self { stmts, tail: tail.map(Box::new) }
    }

    /// `{ tail }`
    #[must_use]
    pub fn of(tail: Expr) -> Self {
        Self::new(Vec::new(), Some(tail))
    }
}
