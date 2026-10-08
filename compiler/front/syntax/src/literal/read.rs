//! Reading literal values from the CST.

use num_bigint::BigInt;
use vun_grammar::{NodeField, NodeKind, TokenKind};
use vun_span::Span;

use super::{Decl, Literal};
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
        match self.cst.anchor(n) {
            Anchor::Token(t) => self.tokens.doc_text(t, self.source),
            Anchor::Missing(_) => None,
        }
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
        nodes.filter_map(|n| self.decl(n, Vec::new(), Vec::new())).collect()
    }

    /// The lines of the module doc comments among `nodes`.
    pub(super) fn module_doc(&self, nodes: impl Iterator<Item = NodeId>) -> Vec<&'s str> {
        let docs = nodes.filter(|&n| self.cst.kind(n) == NodeKind::ModuleDoc);
        docs.filter_map(|n| self.doc_line(n)).collect()
    }

    fn decl(&self, n: NodeId, mut doc: Vec<&'s str>, mut attrs: Vec<Literal<'s>>) -> Option<Decl<'s>> {
        let children = self.children(n);
        match self.cst.kind(n) {
            NodeKind::Doc => {
                doc.push(self.doc_line(n)?);
                self.decl(*children.last()?, doc, attrs)
            }
            NodeKind::Attr => {
                let (decorated, written) = children.split_last()?;
                attrs.extend(
                    written.iter().filter(|&&c| !self.cst.kind(c).is_token()).map(|&c| self.value(c)),
                );
                self.decl(*decorated, doc, attrs)
            }
            // A field of a `struct` literal, its value the default it declares, if any.
            NodeKind::Field => {
                let fields = self.cst.fields(n)?;
                let name = self.name(fields.one(NodeField::Pattern)?)?;
                let value = fields.one(NodeField::Value).map_or(Literal::Opaque, |v| self.value(v));
                Some(Decl { doc, attrs, ..Decl::new(name, value, self.span(n)) })
            }
            NodeKind::Binding | NodeKind::TypedBinding => {
                let fields = self.cst.fields(n)?;
                let name = self.name(fields.one(NodeField::Pattern)?)?;
                let value = fields.one(NodeField::Value);
                let span = self.span(value.unwrap_or(n));
                let value = value.map_or(Literal::Opaque, |v| self.value(v));
                Some(Decl { doc, attrs, ..Decl::new(name, value, span) })
            }
            _ => None,
        }
    }

    fn value(&self, n: NodeId) -> Literal<'s> {
        self.read(n).unwrap_or(Literal::Opaque)
    }

    fn read(&self, n: NodeId) -> Option<Literal<'s>> {
        let children = self.children(n);
        let items = || children.iter().copied().filter(|&c| !self.cst.kind(c).is_token());
        Some(match self.cst.kind(n) {
            NodeKind::DotName => Literal::Name(self.name(*children.first()?)?),
            NodeKind::Hole => Literal::Hole,
            NodeKind::String => Literal::String(lexer::literal(self.token(n)?, self.text(n)?)?),
            NodeKind::Int => Literal::Int(i128::try_from(integer(self.text(n)?)?).ok()?),
            NodeKind::Prefix if self.token(n)? == TokenKind::Minus => match self.read(*children.first()?)? {
                Literal::Int(i) => Literal::Int(-i),
                _ => return None,
            },
            // Attributes mark what they precede: `#[layout.c] struct { ... }` declares as the
            // struct does.
            NodeKind::Attr => self.read(*children.last()?)?,
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
                if inits.iter().all(|&c| self.is_field_init(c)) {
                    Literal::Record(inits.into_iter().filter_map(|c| self.field_init(c)).collect())
                } else {
                    Literal::List(inits.into_iter().map(|c| self.value(c)).collect())
                }
            }
            NodeKind::Enum => {
                let fields = self.cst.fields(n)?;
                let variants =
                    fields.get(NodeField::Variants).iter().filter_map(|&c| self.variant(c, Vec::new()));
                let namespace = match fields.one(NodeField::Namespace) {
                    Some(ns) => self.decls(self.cst.fields(ns)?.get(NodeField::Members).iter().copied()),
                    None => Vec::new(),
                };
                Literal::Enum(variants.collect(), namespace)
            }
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

    /// A field `.name = value` of a record, looking through its doc comments.
    fn field_init(&self, n: NodeId) -> Option<Decl<'s>> {
        self.documented_init(n, Vec::new())
    }

    fn documented_init(&self, n: NodeId, mut doc: Vec<&'s str>) -> Option<Decl<'s>> {
        if self.cst.kind(n) == NodeKind::Doc {
            doc.push(self.doc_line(n)?);
            return self.documented_init(*self.children(n).last()?, doc);
        }
        let fields = self.cst.fields(n)?;
        let (target, value) = (fields.one(NodeField::Target)?, fields.one(NodeField::Value)?);
        let name = self.name(self.cst.fields(target)?.one(NodeField::Property)?)?;
        Some(Decl { doc, ..Decl::new(name, self.value(value), self.span(value)) })
    }

    /// Whether `n` is a field of a record, under its doc comments.
    fn is_field_init(&self, n: NodeId) -> bool {
        match self.cst.kind(n) {
            NodeKind::FieldInit => true,
            NodeKind::Doc => self.cst.children(n).last().is_some_and(|c| self.is_field_init(c)),
            _ => false,
        }
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
            NodeKind::Name => Some(Decl { doc, ..Decl::new(self.name(n)?, Literal::Opaque, self.span(n)) }),
            NodeKind::Call => {
                let name = self.name(*self.children(n).first()?)?;
                Some(Decl { doc, ..Decl::new(name, Literal::Opaque, self.span(n)) })
            }
            _ => None,
        }
    }
}

/// The value of an integer literal, exactly: decimal, or `0x`, `0o`, `0b`, with `_` separators.
#[must_use]
pub fn integer(text: &str) -> Option<BigInt> {
    let (digits, radix) = text.strip_prefix('0').and_then(radix).unwrap_or((text, 10));
    let digits: Vec<u8> = digits.bytes().filter(|&b| char::from(b) != lexer::DIGIT_SEPARATOR).collect();
    BigInt::parse_bytes(&digits, radix)
}

/// The digits after a radix prefix's letter, and the radix.
fn radix(text: &str) -> Option<(&str, u32)> {
    let mut chars = text.chars();
    let prefix = chars.next()?;
    let &(_, radix) = lexer::RADIX_PREFIXES.iter().find(|&&(c, _)| c == prefix)?;
    Some((chars.as_str(), radix))
}
