//! Reading literal declarations as data, without evaluating anything (spec 12, section 2).
//!
//! Tools read `build.ov` manifests and `lang/syntax/grammar.ov` this way. A declaration
//! `name := value;` whose value is made of strings, integers, `.name` literals, tagged values
//! `.name(a, b)`, lists `[a, b]`,
//! anonymous structs `.{ .field = value }` and type literals `enum { ... }` and
//! `struct { name := value; }` reads as the corresponding [`Literal`]. Any other expression reads
//! as [`Literal::Opaque`], so a caller can walk a file whose other values it does not need.

use std::borrow::Cow;

use overt_grammar::{NodeField, NodeKind, TokenKind};

use overt_span::Span;

use crate::cst::{Anchor, Cst, NodeId};
use crate::lexer;
use crate::tokens::Tokens;

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
    /// `enum { a, b(T) }`; the payloads read as `Opaque`.
    Enum(Vec<Decl<'s>>),
    /// `struct { name := value; }`; fields are skipped.
    Namespace(Vec<Decl<'s>>),
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
    /// The value of the field `name` of a record or namespace.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Self> {
        match self {
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

/// The top-level `name := value;` and `name: T = value;` declarations of a parsed file.
#[must_use]
pub fn declarations<'s>(cst: &Cst, tokens: &Tokens, source: &'s str) -> Vec<Decl<'s>> {
    let reader = Reader { cst, tokens, source };
    reader.decls(cst.children(cst.root()))
}

struct Reader<'a, 's> {
    cst: &'a Cst,
    tokens: &'a Tokens,
    source: &'s str,
}

/// Doc comment prefix, with the space that conventionally follows it.
const DOC: &str = "/// ";

impl<'s> Reader<'_, 's> {
    fn text(&self, n: NodeId) -> Option<&'s str> {
        match self.cst.anchor(n) {
            Anchor::Token(t) => Some(self.tokens.range(t).of(self.source)),
            Anchor::Missing(_) => None,
        }
    }

    fn token(&self, n: NodeId) -> Option<TokenKind> {
        match self.cst.anchor(n) {
            Anchor::Token(t) => Some(self.tokens.kind(t)),
            Anchor::Missing(_) => None,
        }
    }

    fn doc_line(&self, n: NodeId) -> Option<&'s str> {
        let line = self.text(n)?;
        line.strip_prefix(DOC).or_else(|| line.strip_prefix(DOC.trim_end()))
    }

    fn children(&self, n: NodeId) -> Vec<NodeId> {
        self.cst.children(n).collect()
    }

    /// A name leaf, without the escape of `\name`.
    fn name(&self, n: NodeId) -> Option<&'s str> {
        let text = self.text(n).filter(|_| self.cst.kind(n) == NodeKind::Name)?;
        Some(text.strip_prefix('\\').unwrap_or(text))
    }

    /// Declarations among `nodes`, looking through doc comments and attributes.
    fn decls(&self, nodes: impl Iterator<Item = NodeId>) -> Vec<Decl<'s>> {
        nodes.filter_map(|n| self.decl(n, Vec::new())).collect()
    }

    fn decl(&self, n: NodeId, mut doc: Vec<&'s str>) -> Option<Decl<'s>> {
        let children = self.children(n);
        match self.cst.kind(n) {
            NodeKind::Doc => {
                doc.push(self.doc_line(n)?);
                self.decl(*children.last()?, doc)
            }
            NodeKind::Attr => self.decl(*children.last()?, doc),
            NodeKind::Binding | NodeKind::TypedBinding => {
                let fields = self.cst.fields(n)?;
                let name = self.name(fields.one(NodeField::Pattern)?)?;
                let value = fields.one(NodeField::Value);
                let span = self.span(value.unwrap_or(n));
                Some(Decl { name, doc, value: value.map_or(Literal::Opaque, |v| self.value(v)), span })
            }
            _ => None,
        }
    }

    fn value(&self, n: NodeId) -> Literal<'s> {
        self.read(n).unwrap_or(Literal::Opaque)
    }

    fn read(&self, n: NodeId) -> Option<Literal<'s>> {
        let children = self.children(n);
        let items = || children.iter().copied().filter(|&c| self.cst.kind(c) != NodeKind::Punct);
        Some(match self.cst.kind(n) {
            NodeKind::DotName => Literal::Name(self.name(*children.first()?)?),
            NodeKind::String => Literal::String(lexer::literal(self.token(n)?, self.text(n)?)?),
            NodeKind::Int => Literal::Int(integer(self.text(n)?)?),
            NodeKind::Prefix if self.token(n)? == TokenKind::Minus => match self.read(*children.first()?)? {
                Literal::Int(i) => Literal::Int(-i),
                _ => return None,
            },
            NodeKind::Array => Literal::List(items().map(|c| self.value(c)).collect()),
            NodeKind::Call => {
                let mut items = items();
                let tag = items.next().filter(|&c| self.cst.kind(c) == NodeKind::DotName)?;
                let Literal::Name(tag) = self.read(tag)? else { return None };
                Literal::Tagged(tag, items.map(|c| self.value(c)).collect())
            }
            NodeKind::StructLit => {
                let inits: Vec<_> = items().collect();
                if inits.iter().all(|&c| self.cst.kind(c) == NodeKind::FieldInit) {
                    Literal::Record(inits.into_iter().filter_map(|c| self.field_init(c)).collect())
                } else {
                    Literal::List(inits.into_iter().map(|c| self.value(c)).collect())
                }
            }
            NodeKind::Enum => Literal::Enum(items().filter_map(|c| self.variant(c, Vec::new())).collect()),
            NodeKind::Struct => Literal::Namespace(self.decls(items())),
            _ => return None,
        })
    }

    fn field_init(&self, n: NodeId) -> Option<Decl<'s>> {
        let fields = self.cst.fields(n)?;
        let (target, value) = (fields.one(NodeField::Target)?, fields.one(NodeField::Value)?);
        let name = self.name(self.cst.fields(target)?.one(NodeField::Property)?)?;
        Some(Decl { name, doc: Vec::new(), value: self.value(value), span: self.span(value) })
    }

    fn span(&self, n: NodeId) -> Span {
        self.cst.span(n, self.tokens)
    }

    fn variant(&self, n: NodeId, mut doc: Vec<&'s str>) -> Option<Decl<'s>> {
        match self.cst.kind(n) {
            NodeKind::Doc => {
                doc.push(self.doc_line(n)?);
                self.variant(*self.children(n).last()?, doc)
            }
            NodeKind::Name => {
                Some(Decl { name: self.name(n)?, doc, value: Literal::Opaque, span: self.span(n) })
            }
            NodeKind::Call => {
                let name = self.name(*self.children(n).first()?)?;
                Some(Decl { name, doc, value: Literal::Opaque, span: self.span(n) })
            }
            _ => None,
        }
    }
}

/// Integer prefixes after a leading `0`, with their radix.
const RADIXES: [(&str, u32); 3] = [("0x", 16), ("0o", 8), ("0b", 2)];
/// Digit separator in integer literals.
const SEPARATOR: char = '_';

fn integer(text: &str) -> Option<i128> {
    let (digits, radix) = RADIXES
        .iter()
        .find_map(|&(prefix, radix)| Some((text.strip_prefix(prefix)?, radix)))
        .unwrap_or((text, 10));
    let digits: String = digits.chars().filter(|&c| c != SEPARATOR).collect();
    i128::from_str_radix(&digits, radix).ok()
}

#[cfg(test)]
mod tests;
