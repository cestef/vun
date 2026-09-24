//! Reading literal declarations as data, without evaluating anything (spec 12, section 2).
//!
//! Tools read `build.vun` manifests and `lang/syntax/grammar.vun` this way. A declaration
//! `name := value;` whose value is made of strings, integers, `.name` literals, tagged values
//! `.name(a, b)`, lists `[a, b]`,
//! anonymous structs `.{ .field = value }`, typed ones `a.T{ .field = value }`, calls of paths
//! with literal arguments `a.f(x)`, and type literals `enum { ... }` and
//! `struct { name := value; }` reads as the corresponding [`Literal`]. Any other expression reads
//! as [`Literal::Opaque`], so a caller can walk a file whose other values it does not need.

mod insert;
mod print;
mod read;
#[cfg(test)]
mod tests;

use std::borrow::Cow;

use vun_span::Span;

use crate::cst::Cst;
use crate::tokens::Tokens;
pub(crate) use print::{INDENT, Writer};
pub use print::{Unwritable, WIDTH};
use read::Reader;
pub use read::integer;

/// Starts a doc comment line, with the space after it.
pub(crate) const DOC: &str = "/// ";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Literal<'s> {
    /// `.name`
    Name(&'s str),
    /// `.name(a, b)`: a name with a payload.
    Tagged(&'s str, Vec<Literal<'s>>),
    String(Cow<'s, str>),
    Int(i128),
    /// `[a, b]`, or `.{ a, b }` with positional items.
    List(Vec<Literal<'s>>),
    /// `.{ .name = value }`
    Record(Vec<Decl<'s>>),
    /// `a.T{ .name = value }`: the type's path and the record.
    Typed(Vec<&'s str>, Box<Literal<'s>>),
    /// `a.b.c`: a declaration or namespace named by its path, which a reader interprets.
    Path(Vec<&'s str>),
    /// `a.f(x, y)`: a call of a path, which a reader interprets by the path.
    Call(Vec<&'s str>, Vec<Literal<'s>>),
    /// `enum { a, b(T) }`; the payloads read as `Opaque`.
    Enum(Vec<Decl<'s>>),
    /// `struct { name := value; }`; fields are skipped.
    Namespace(Vec<Decl<'s>>),
    /// `_`: provided from outside (spec 01, section 3).
    Hole,
    Opaque,
}

/// A named value with its doc comment lines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decl<'s> {
    pub name: &'s str,
    pub doc: Vec<&'s str>,
    pub value: Literal<'s>,
    /// The source of the value, or of the whole declaration when it has none.
    pub span: Span,
}

impl<'s> Literal<'s> {
    /// A list of strings.
    pub fn strings(items: impl IntoIterator<Item = &'s str>) -> Self {
        Self::List(items.into_iter().map(|s| Self::String(Cow::Borrowed(s))).collect())
    }

    /// The value of the field `name` of a record, typed record or namespace.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Self> {
        match self {
            Self::Typed(_, record) => record.get(name),
            Self::Record(decls) | Self::Namespace(decls) | Self::Enum(decls) => {
                decls.iter().find(|d| d.name == name).map(|d| &d.value)
            }
            _ => None,
        }
    }

    #[must_use]
    pub fn as_list(&self) -> Option<&[Self]> {
        match self {
            Self::List(items) => Some(items),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_name(&self) -> Option<&'s str> {
        match *self {
            Self::Name(name) => Some(name),
            _ => None,
        }
    }

    /// The path and arguments of a call.
    #[must_use]
    pub fn as_call(&self) -> Option<(&[&'s str], &[Self])> {
        match self {
            Self::Call(path, args) => Some((path, args)),
            _ => None,
        }
    }

    /// The tag and payload of a tagged value.
    #[must_use]
    pub fn as_tagged(&self) -> Option<(&'s str, &[Self])> {
        match self {
            Self::Tagged(tag, payload) => Some((tag, payload)),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(text) => Some(text),
            _ => None,
        }
    }

    /// The declarations of a record, enum or namespace.
    #[must_use]
    pub fn decls(&self) -> Option<&[Decl<'s>]> {
        match self {
            Self::Record(decls) | Self::Enum(decls) | Self::Namespace(decls) => Some(decls),
            _ => None,
        }
    }
}

impl Cst {
    /// The top-level `name := value;` and `name: T = value;` declarations of a parsed file.
    #[must_use]
    pub fn declarations<'s>(&self, tokens: &Tokens, source: &'s str) -> Vec<Decl<'s>> {
        let reader = Reader { cst: self, tokens, source };
        reader.decls(self.children(self.root()))
    }
}
