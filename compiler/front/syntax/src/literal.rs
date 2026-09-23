//! Reading literal declarations as data, without evaluating anything (spec 12, section 2).
//!
//! Tools read `build.ov` manifests and `lang/syntax/grammar.ov` this way. A declaration
//! `name := value;` whose value is made of strings, integers, `.name` literals, lists `[a, b]`,
//! anonymous structs `.{ .field = value }` and type literals `enum { ... }` and
//! `struct { name := value; }` reads as the corresponding [`Literal`]. Any other expression reads
//! as [`Literal::Opaque`], so a caller can walk a file whose other values it does not need.

use std::borrow::Cow;

use overt_grammar::{NodeKind, TokenKind};

use crate::cst::{Anchor, Cst, NodeId};
use crate::lexer;
use crate::tokens::Tokens;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Literal<'s> {
    /// `.name`
    Name(&'s str),
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
            NodeKind::Binding => {
                let name =
                    self.name(children.iter().copied().find(|&c| self.cst.kind(c) != NodeKind::Mut)?)?;
                let value = match self.token(n)? {
                    TokenKind::ColonEq => children.last().copied(),
                    _ => children
                        .iter()
                        .position(|&c| self.token(c) == Some(TokenKind::Eq))
                        .map(|i| children[i + 1]),
                };
                Some(Decl { name, doc, value: value.map_or(Literal::Opaque, |v| self.value(v)) })
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
            NodeKind::StructLit if self.token(n)? == TokenKind::DotLBrace => {
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
        let [field, value] = self.children(n)[..] else { return None };
        let [name] = self.children(field)[..] else { return None };
        Some(Decl { name: self.name(name)?, doc: Vec::new(), value: self.value(value) })
    }

    fn variant(&self, n: NodeId, mut doc: Vec<&'s str>) -> Option<Decl<'s>> {
        match self.cst.kind(n) {
            NodeKind::Doc => {
                doc.push(self.doc_line(n)?);
                self.variant(*self.children(n).last()?, doc)
            }
            NodeKind::Name => Some(Decl { name: self.name(n)?, doc, value: Literal::Opaque }),
            NodeKind::Call => {
                Some(Decl { name: self.name(*self.children(n).first()?)?, doc, value: Literal::Opaque })
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
mod tests {
    use super::*;
    use crate::{lexer::lex, parser::parse};
    use overt_diag::{DiagCtxt, DiagCx};
    use overt_origin::OriginTable;
    use overt_span::SourceFiles;

    fn read(text: &str) -> Vec<Decl<'_>> {
        let file = SourceFiles::new().add("t.ov", text);
        let (mut diag, mut origins) = (DiagCtxt::new(), OriginTable::new());
        let mut cx = DiagCx { diag: &mut diag, origins: &mut origins };
        let tokens = lex(file, text, &mut cx);
        let cst = parse(&tokens, text, &mut cx);
        assert_eq!(diag.error_count(), 0);
        declarations(&cst, &tokens, text)
    }

    #[test]
    fn reads_literals_and_skips_the_rest() {
        let text = "/// doc\nxs := [.a, .\\unsafe, \"s\", -0x1_0];\nr: T = .{ .k = 1, .l = .{ 2 } };\n\
                    E := enum { /// v\n a, b(u8) };\nS := struct { f: u8, d := f(); };\n";
        let decls = read(text);
        let names: Vec<_> = decls.iter().map(|d| d.name).collect();
        assert_eq!(names, ["xs", "r", "E", "S"]);
        assert_eq!(decls[0].doc, ["doc"]);
        assert_eq!(
            decls[0].value,
            Literal::List(vec![
                Literal::Name("a"),
                Literal::Name("unsafe"),
                Literal::String("s".into()),
                Literal::Int(-16)
            ])
        );
        assert_eq!(decls[1].value.get("l"), Some(&Literal::List(vec![Literal::Int(2)])));
        let variants = decls[2].value.decls().unwrap();
        assert_eq!((variants[0].name, &variants[0].doc[..], variants[1].name), ("a", &["v"][..], "b"));
        let members = decls[3].value.decls().unwrap();
        assert_eq!((members.len(), members[0].name, &members[0].value), (1, "d", &Literal::Opaque));
    }
}
