//! Making Vun source: declarations and expressions as values, which one printer writes, so that
//! tools generating code (`vun util ffi bind`, the smith) never assemble text. Types are expressions
//! (spec 02, section 1) and so are patterns (spec 01, section 6), so one [`Expr`] serves for
//! values, types and patterns, and declarations of `lang` and `std` are named by their handles in
//! `vun-items`.
//!
//! What is made is meant to be read back: every form prints as the parser reads it, parenthesized
//! where the grammar's levels or its head and statement rules ask, names are escaped where they
//! are keywords, and punctuation is spelled by the grammar's token table.

mod expr;
mod integer;
mod kinds;
mod level;
mod parts;
mod print;
#[cfg(test)]
mod tests;

pub use expr::Expr;
pub use integer::Integer;
pub use parts::{Arm, Block, Borrow, Field, Function, Init, Member, Param, Property, Variant};

/// A source file: its module doc lines, the namespaces it spreads (`..a.b;`), then its declarations.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct File {
    pub doc: Vec<String>,
    pub spreads: Vec<Vec<String>>,
    pub items: Vec<Item>,
}

/// A declaration with its docs and attributes: `#[attrs] mut name: ty = value;`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub doc: Vec<String>,
    pub attrs: Vec<Expr>,
    pub bind: Bind,
}

/// A binding: `mut pattern: ty = value`, or `pattern := value` without a type. Without a value
/// it is uninitialized (`mut buf: [4096]u8`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bind {
    pub mutable: bool,
    pub pattern: Expr,
    pub ty: Option<Expr>,
    pub value: Option<Expr>,
}

impl Bind {
    /// `name := value`
    #[must_use]
    pub fn new(name: &str, value: Expr) -> Self {
        Self { mutable: false, pattern: Expr::local(name), ty: None, value: Some(value) }
    }

    /// `name: ty = value`
    #[must_use]
    pub fn typed(name: &str, ty: Expr, value: Expr) -> Self {
        Self { ty: Some(ty), ..Self::new(name, value) }
    }

    /// `mut name: ty = value`
    #[must_use]
    pub fn mutable(name: &str, ty: Expr, value: Expr) -> Self {
        Self { mutable: true, ..Self::typed(name, ty, value) }
    }

    /// The declaration binding this, without docs or attributes.
    #[must_use]
    pub fn item(self) -> Item {
        Item { doc: Vec::new(), attrs: Vec::new(), bind: self }
    }
}

impl Item {
    /// This declaration annotated with the type `ty`.
    #[must_use]
    pub fn typed(mut self, ty: Expr) -> Self {
        self.bind.ty = Some(ty);
        self
    }

    /// This declaration, mutable or not.
    #[must_use]
    pub fn mutable(mut self, mutable: bool) -> Self {
        self.bind.mutable = mutable;
        self
    }
}

/// Attributes as written before what they apply to: `#[a, b]`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Attributes(pub Vec<Expr>);

impl Attributes {
    /// Their source.
    #[must_use]
    pub fn source(&self) -> String {
        print::attributes(&self.0)
    }
}

impl File {
    /// The source of this file.
    #[must_use]
    pub fn source(&self) -> String {
        print::file(self)
    }
}

impl Expr {
    /// The source of this expression on its own: a type, a pattern, a value.
    #[must_use]
    pub fn source(&self) -> String {
        print::expr(self)
    }
}
