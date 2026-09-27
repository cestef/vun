//! C99 as data: what the C backend builds from LIR, printed once by [`File`]'s `Display`
//! (spec 13, section 8.2). Only the forms the backend writes exist. No header is included: the
//! file declares its own fixed-width integers from the target's C types.

mod ident;
mod print;
mod ty;

pub use ident::{Builtin, Helper, Ident, Names, Own};
pub use ty::{Base, Suffix, Ty};

use vun_backend::Line;
use vun_intern::Symbol;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expr {
    Name(Ident),
    /// An unsigned integer constant: `5u`, `5ul`, `5ull`.
    Literal {
        value: u128,
        suffix: Suffix,
    },
    /// A plain decimal integer, as an offset or a byte is written.
    Small(i64),
    /// A finite float as an exact hexadecimal literal, `bits` wide, with these bits.
    Float {
        bits: u32,
        value: u64,
    },
    Unary(Unary, Box<Expr>),
    Binary(Binary, Box<Expr>, Box<Expr>),
    Cast(Ty, Box<Expr>),
    Call(Box<Expr>, Vec<Expr>),
    Select(Box<Expr>, Box<Expr>, Box<Expr>),
    Sizeof(Ty),
    /// A braced initializer.
    Init(Vec<Expr>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unary {
    Deref,
    Address,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Binary {
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
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Either,
}

impl Binary {
    /// Whether the result is a truth value, an `int` 0 or 1.
    #[must_use]
    pub const fn is_comparison(self) -> bool {
        matches!(self, Self::Eq | Self::Ne | Self::Lt | Self::Le | Self::Gt | Self::Ge | Self::Either)
    }
}

impl Expr {
    /// `self` converted to `ty`. A conversion to an object pointer of an object pointer replaces
    /// the conversion that made it: only addresses and arrays are converted to object pointers,
    /// integers and functions going through the integer as wide as an address first.
    #[must_use]
    pub fn cast(self, ty: Ty) -> Self {
        match self {
            Self::Cast(Ty::Pointer(_), inner) if matches!(ty, Ty::Pointer(_)) => Self::Cast(ty, inner),
            e => Self::Cast(ty, Box::new(e)),
        }
    }

    #[must_use]
    pub fn binary(op: Binary, a: Self, b: Self) -> Self {
        Self::Binary(op, Box::new(a), Box::new(b))
    }

    #[must_use]
    pub fn unary(op: Unary, e: Self) -> Self {
        Self::Unary(op, Box::new(e))
    }

    /// A byte offset or count as a plain literal.
    ///
    /// # Panics
    /// Panics past `i64::MAX`, which no offset reaches.
    #[must_use]
    pub fn small(n: u64) -> Self {
        Self::Small(i64::try_from(n).expect("offsets fit i64"))
    }

    /// The object of type `ty` at the address `self`.
    #[must_use]
    pub fn at(self, ty: Ty) -> Self {
        Self::unary(Unary::Deref, self.cast(ty.pointer()))
    }

    /// `&self` as an address.
    #[must_use]
    pub fn address_of(self) -> Self {
        Self::unary(Unary::Address, self).cast(Ty::address())
    }

    /// Itself where only its truth is read: a comparison needs no conversion to `u8`.
    #[must_use]
    pub fn truth(self) -> Self {
        match self {
            Self::Cast(_, inner) if matches!(*inner, Self::Binary(op, ..) if op.is_comparison()) => *inner,
            e => e,
        }
    }
}

/// A declaration: a type, a name, and an array length.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decl {
    pub ty: Ty,
    pub name: Ident,
    pub len: Option<u64>,
}

impl Decl {
    #[must_use]
    pub fn new(ty: Ty, name: Ident) -> Self {
        Self { ty, name, len: None }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stmt {
    /// `#line`, placing the statements after it.
    Line(Line),
    Local(Decl),
    Expr(Expr),
    Assign(Expr, Expr),
    Label(Ident),
    Goto(Ident),
    If(Expr, Vec<Stmt>, Vec<Stmt>),
    Switch(Expr, Vec<(Expr, Vec<Stmt>)>, Vec<Stmt>),
    Return(Option<Expr>),
    /// `for (;;) {}`: control never gets here.
    Halt,
}

impl Stmt {
    /// Adds the labels the `goto`s of `stmts` name to `out`.
    pub fn targets(stmts: &[Self], out: &mut std::collections::BTreeSet<Ident>) {
        for stmt in stmts {
            match stmt {
                Self::Goto(label) => {
                    out.insert(*label);
                }
                Self::If(_, then, otherwise) => {
                    Self::targets(then, out);
                    Self::targets(otherwise, out);
                }
                Self::Switch(_, cases, default) => {
                    for (_, body) in cases {
                        Self::targets(body, out);
                    }
                    Self::targets(default, out);
                }
                _ => {}
            }
        }
    }
}

/// Who sees a file-scope name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Storage {
    /// `static`: this file only.
    Static,
    /// Declared here, defined elsewhere.
    Extern,
    /// Defined here and seen by the linker.
    Public,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Aggregate {
    Struct,
    Union,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proto {
    pub storage: Storage,
    pub result: Ty,
    pub name: Ident,
    pub params: Vec<Decl>,
}

/// Where GNU C places an object beyond what C99 says (`__attribute__`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Attribute {
    /// At an address a multiple of these bytes.
    Aligned(u64),
    /// In the link section of this name.
    Section(Symbol),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Item {
    /// `#include "name"`: a header of the same program.
    Include(&'static str),
    Typedef(Decl),
    Aggregate {
        kind: Aggregate,
        tag: Ident,
        fields: Vec<Decl>,
    },
    /// `typedef char name[holds ? 1 : -1];`: a check C99 makes while compiling.
    Check {
        name: Ident,
        holds: Expr,
    },
    Declare(Proto),
    Define(Proto, Vec<Stmt>),
    Object {
        storage: Storage,
        constant: bool,
        decl: Decl,
        attributes: Vec<Attribute>,
        init: Option<Expr>,
    },
}

/// A translation unit, and how its names are spelled.
pub struct File {
    pub names: Names,
    pub items: Vec<Item>,
}
