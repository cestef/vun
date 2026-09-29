//! The grammar's files as written, read by type: the kinds of tokens and nodes, the tables the
//! generator interprets, and the editors' facts. Tables it only copies into Rust are read by
//! shape instead (`tables.rs`), so a new one needs no change here.

use vun_syntax::literal::{Expected, Fields, FromLiteral, Literal, Mismatch, Name, Path};

use crate::grammar::sublime::{Recognized, Scope};
use crate::grammar::tree_sitter::queries::Capture;

#[derive(FromLiteral)]
pub(super) struct Written {
    #[literal(name = "TokenKind")]
    pub(super) tokens: Fields<Vec<Entry>>,
    #[literal(name = "NodeKind")]
    pub(super) nodes: Fields<Vec<Entry>>,
    pub(super) parts: Vec<Name>,
    pub(super) admits: Vec<Contents>,
    pub(super) holds: Vec<Contents>,
    pub(super) operand_contexts: Vec<Contents>,
    pub(super) preferred: Vec<Vec<Name>>,
    pub(super) unterminated: Vec<Name>,
    pub(super) transparent: Vec<Transparent>,
    pub(super) line_ends: Vec<Name>,
    pub(super) precedence: Vec<Vec<Name>>,
    pub(super) infix: Vec<Level>,
    pub(super) delimiters: Vec<Delimiter>,
    pub(super) contextual: Vec<Contextual>,
}

/// The editors' facts of `editors.vun`.
#[derive(FromLiteral)]
pub(super) struct WrittenEditors {
    pub(super) highlights: Highlights,
    pub(super) outline: Outline,
    pub(super) text_objects: Captures,
    pub(super) sublime: Sublime,
    pub(super) tree_sitter: TreeSitter,
}

#[derive(FromLiteral)]
pub(super) struct TreeSitter {
    pub(super) error: Name,
    pub(super) end: Path,
    pub(super) word: Name,
    pub(super) extras: Vec<Name>,
}

/// A kind's entry: `.name`, a spelling `"text"` named after itself, `.{ .name, .text }`, or
/// `.name(terms...)` with the kind's production.
pub(crate) struct Entry {
    pub(crate) name: String,
    pub(crate) text: Option<String>,
    pub(crate) production: Option<Vec<Term>>,
}

#[derive(FromLiteral)]
struct Spelled {
    name: Name,
    text: String,
}

impl FromLiteral for Entry {
    fn from_literal(value: &Literal<'_>) -> Result<Self, Mismatch> {
        let plain = |name: &str, text: Option<String>| Self { name: name.to_owned(), text, production: None };
        match value {
            Literal::Name(name) => Ok(plain(name, None)),
            Literal::String(text) => Ok(plain(text, Some(text.to_string()))),
            Literal::Record(_) => value.read::<Spelled>().map(|s| plain(&s.name.0, Some(s.text))),
            Literal::Tagged(name, _) => {
                let terms = value.arguments(name, 0, true)?.map(|args| args.from(0)).transpose()?;
                Ok(Self { name: (*name).to_owned(), text: None, production: terms })
            }
            _ => Err(Mismatch::new(Expected::Form)),
        }
    }
}

/// A term of a production as written (the forms are in the doc of `NodeKind`).
#[derive(FromLiteral)]
pub(crate) enum Term {
    Token(#[literal(rest)] Vec<TokenRef>),
    Word(Path),
    Optional(#[literal(rest)] Vec<Term>),
    Many(#[literal(rest)] Vec<Term>),
    Separated(Box<Term>, Box<Term>),
    Choice(#[literal(rest)] Vec<Term>),
    Seq(#[literal(rest)] Vec<Term>),
    Of(Name, Path),
    Starting(Name, Path),
    #[literal(untagged)]
    Text(String),
    #[literal(untagged)]
    Field(Name),
}

/// A token kind, or a list or table of them.
#[derive(FromLiteral)]
pub(crate) enum TokenRef {
    #[literal(untagged)]
    Kind(Name),
    #[literal(untagged)]
    Table(Path),
}

/// A row of `admits`, `holds` or `operand_contexts`.
#[derive(FromLiteral)]
pub(super) struct Contents {
    pub(super) fields: Vec<Name>,
    pub(super) kinds: Vec<Name>,
}

/// A row of `transparent`.
#[derive(FromLiteral)]
pub(super) struct Transparent {
    pub(super) kind: Name,
    pub(super) tokens: Vec<Name>,
}

/// A row of `infix`.
#[derive(FromLiteral)]
pub(super) struct Level {
    pub(super) tokens: Vec<Name>,
}

#[derive(FromLiteral)]
pub(super) struct Delimiter {
    pub(super) open: Name,
    pub(super) close: Name,
}

#[derive(FromLiteral)]
pub(super) struct Contextual {
    pub(super) text: String,
    pub(super) context: Recognized,
}

#[derive(FromLiteral)]
pub(super) struct Highlights {
    pub(super) name: Name,
    pub(super) tokens: Vec<TokenCapture>,
    pub(super) nodes: Vec<NodeCapture>,
    pub(super) places: Vec<Place>,
    pub(super) bindings: Vec<Binding>,
}

/// Nodes captured whole, and where they stand.
#[derive(FromLiteral)]
pub(super) struct Captures {
    pub(super) nodes: Vec<NodeCapture>,
    pub(super) places: Vec<Place>,
}

#[derive(FromLiteral)]
pub(super) struct TokenCapture {
    pub(super) tokens: Path,
    pub(super) capture: Capture,
}

#[derive(FromLiteral)]
pub(super) struct NodeCapture {
    pub(super) nodes: Vec<Name>,
    pub(super) capture: Capture,
}

#[derive(FromLiteral)]
pub(super) struct Place {
    pub(super) kind: Name,
    pub(super) field: Name,
    pub(super) inner: Option<Name>,
    pub(super) node: Name,
    pub(super) capture: Capture,
}

#[derive(FromLiteral)]
pub(super) struct Binding {
    pub(super) kinds: Vec<Name>,
    pub(super) name: Name,
    pub(super) value: Name,
    pub(super) values: Vec<Name>,
    pub(super) capture: Capture,
}

#[derive(FromLiteral)]
pub(super) struct Outline {
    pub(super) kinds: Vec<Name>,
    pub(super) field: Name,
    pub(super) node: Name,
}

#[derive(FromLiteral)]
pub(super) struct Sublime {
    pub(super) tables: Vec<TokenScope>,
    pub(super) special: Vec<SpecialScope>,
    pub(super) keywords: Path,
    pub(super) qualifiers: Path,
    pub(super) modes: Path,
    pub(super) flows: Path,
    pub(super) punctuation: Path,
    pub(super) member: Name,
    pub(super) call: Name,
    pub(super) binds: Name,
    pub(super) borrow: Name,
    pub(super) block: Name,
}

#[derive(FromLiteral)]
pub(super) struct TokenScope {
    pub(super) tokens: Path,
    pub(super) scope: Scope,
}

#[derive(FromLiteral)]
pub(super) struct SpecialScope {
    pub(super) token: Name,
    pub(super) scope: Scope,
}
