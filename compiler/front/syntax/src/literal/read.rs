//! Reading literal values from the CST.

use vun_grammar::{NodeField, NodeKind, TokenKind};
use vun_span::Span;

use super::{DOC, Decl, Literal};
use crate::cst::{Anchor, Cst, NodeId};
use crate::lexer;
use crate::tokens::Tokens;

pub(super) struct Reader<'a, 's> {
    pub(super) cst: &'a Cst,
    pub(super) tokens: &'a Tokens,
    pub(super) source: &'s str,
}

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
    pub(super) fn decls(&self, nodes: impl Iterator<Item = NodeId>) -> Vec<Decl<'s>> {
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
            // A field of a `struct` literal is a member without a value of its own.
            NodeKind::Field => {
                let name = self.name(self.cst.fields(n)?.one(NodeField::Pattern)?)?;
                Some(Decl { name, doc, value: Literal::Opaque, span: self.span(n) })
            }
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
            NodeKind::Hole => Literal::Hole,
            NodeKind::String => Literal::String(lexer::literal(self.token(n)?, self.text(n)?)?),
            NodeKind::Int => Literal::Int(integer(self.text(n)?)?),
            NodeKind::Prefix if self.token(n)? == TokenKind::Minus => match self.read(*children.first()?)? {
                Literal::Int(i) => Literal::Int(-i),
                _ => return None,
            },
            // A borrow of a literal is the literal: `&[.a, .b]` lists as `[.a, .b]` does.
            NodeKind::Prefix if self.token(n)? == TokenKind::Amp => self.read(*children.last()?)?,
            NodeKind::Name | NodeKind::Member => Literal::Path(self.path(n)?),
            NodeKind::Array => Literal::List(items().map(|c| self.value(c)).collect()),
            NodeKind::Call => {
                let mut items = items();
                let callee = items.next()?;
                let args = items.map(|c| self.value(c)).collect();
                match self.cst.kind(callee) {
                    NodeKind::DotName => {
                        let Literal::Name(tag) = self.read(callee)? else { return None };
                        Literal::Tagged(tag, args)
                    }
                    _ => Literal::Call(self.path(callee)?, args),
                }
            }
            NodeKind::TypedLit => {
                let fields = self.cst.fields(n)?;
                let ty = self.path(fields.one(NodeField::Type)?)?;
                let record = fields.get(NodeField::Fields).iter().map(|&c| self.field_init(c));
                Literal::Typed(ty, Box::new(Literal::Record(record.collect::<Option<_>>()?)))
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
            NodeKind::Fn => {
                let body = self.cst.fields(n)?.one(NodeField::Block)?;
                let statements = self.cst.fields(body)?;
                let [made] = statements.get(NodeField::Statements) else { return None };
                Literal::Made(Box::new(self.read(*made)?))
            }
            _ => return None,
        })
    }

    /// A name or a chain of members: `a.b.c`.
    fn path(&self, n: NodeId) -> Option<Vec<&'s str>> {
        match self.cst.kind(n) {
            NodeKind::Name => Some(vec![self.name(n)?]),
            NodeKind::Member => {
                let fields = self.cst.fields(n)?;
                let mut path = self.path(fields.one(NodeField::Object)?)?;
                path.push(self.name(fields.one(NodeField::Property)?)?);
                Some(path)
            }
            _ => None,
        }
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

/// The value of an integer literal: decimal, or `0x`, `0o`, `0b`, with `_` separators.
#[must_use]
pub fn integer(text: &str) -> Option<i128> {
    let (digits, radix) = RADIXES
        .iter()
        .find_map(|&(prefix, radix)| Some((text.strip_prefix(prefix)?, radix)))
        .unwrap_or((text, 10));
    let digits: String = digits.chars().filter(|&c| c != SEPARATOR).collect();
    i128::from_str_radix(&digits, radix).ok()
}
