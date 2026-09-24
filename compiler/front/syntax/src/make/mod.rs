//! Making Vun source: declarations and expressions as values, which one printer writes, so that
//! tools generating code (`vun tool bind`) never assemble text. Types are expressions (spec 02,
//! section 1), so one [`Expr`] serves for values and types, and declarations of `lang` and `std`
//! are named by their handles in `vun-items`.
//!
//! What is made is meant to be read back: every form prints as the parser reads it, names are
//! escaped where they are keywords, and punctuation is spelled by the grammar's token table.

mod print;
#[cfg(test)]
mod tests;

use vun_grammar::TokenKind;
use vun_items::Item as Decl;

/// A source file: its module doc lines, then its declarations.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct File {
    pub doc: Vec<String>,
    pub items: Vec<Item>,
}

/// `#[attrs] mut name: ty = value;`, or `name := value;` without a type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub doc: Vec<String>,
    pub attrs: Vec<Expr>,
    pub mutable: bool,
    pub name: String,
    pub ty: Option<Expr>,
    pub value: Expr,
}

/// `name: ty`, a field of a struct type or a parameter of a function type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    pub ty: Expr,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expr {
    Int(i128),
    String(String),
    /// `_`: provided from outside (spec 01, section 3).
    Hole,
    /// `()`
    Unit,
    /// `a.b.c`
    Path(Vec<String>),
    /// `callee(args)`
    Call(Box<Expr>, Vec<Expr>),
    /// A prefix operator, as `?T`, `*T` and `unsafe F`: the token of a table of the grammar.
    Prefix(TokenKind, Box<Expr>),
    /// `[len]item`
    Array(Box<Expr>, Box<Expr>),
    /// `#[attrs] inner`
    Attributed(Vec<Expr>, Box<Expr>),
    /// `(params) -> result`, a function type; `None` for the result `()`.
    Fn(Vec<Field>, Option<Box<Expr>>),
    /// `struct { fields }`
    Struct(Vec<Field>),
}

impl Expr {
    /// The declaration `decl`, by its path: `lang.int.unsigned`.
    #[must_use]
    pub fn decl(decl: Decl) -> Self {
        Self::Path(decl.path().iter().map(|&n| n.to_owned()).collect())
    }

    /// A name declared in the file made: a local path of one segment.
    #[must_use]
    pub fn local(name: &str) -> Self {
        Self::Path(vec![name.to_owned()])
    }

    /// `callee(args)`
    #[must_use]
    pub fn call(callee: Self, args: Vec<Self>) -> Self {
        Self::Call(Box::new(callee), args)
    }

    /// `token inner`
    #[must_use]
    pub fn prefix(token: TokenKind, inner: Self) -> Self {
        Self::Prefix(token, Box::new(inner))
    }
}

impl File {
    /// The source of this file.
    #[must_use]
    pub fn source(&self) -> String {
        print::file(self)
    }
}
