//! Writing literal values as Vun source, the inverse of reading them, so that tools change
//! manifests through typed values rather than formatted text.

use std::fmt::{self, Write as _};

use vun_grammar::TokenKind;

use super::{Decl, Literal};
use crate::lexer::{ESCAPE, SIMPLE_ESCAPES, is_ident};

/// Why a value has no literal source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unwritable {
    /// An expression that was not read as a literal.
    Opaque,
    /// An `enum` or `struct` type, whose members were read without their values.
    Type,
    /// A name that no escape makes a name.
    Name,
    /// A field inserted into something other than a record.
    NotRecord,
}

impl fmt::Display for Unwritable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Opaque => "the value is not a literal",
            Self::Type => "a type literal is read without its values",
            Self::Name => "the name is not an identifier",
            Self::NotRecord => "fields are inserted into records only",
        })
    }
}

impl std::error::Error for Unwritable {}

/// The width lines are kept within, and the indentation of each nesting level, until `vun fmt`
/// owns the layout.
pub const WIDTH: usize = 100;
pub(crate) const INDENT: &str = "    ";

mod layout;

pub(super) use layout::Item;

impl Literal<'_> {
    /// The source that reads back as this value, on one line.
    ///
    /// # Errors
    /// Returns why the value has no literal source.
    pub fn source(&self) -> Result<String, Unwritable> {
        let mut w = Writer::default();
        w.value(self)?;
        Ok(w.out)
    }

    /// The source of this value starting `column` characters into a line indented by `indent`:
    /// one line when it fits within [`WIDTH`], otherwise lists and records hold one item per line.
    ///
    /// # Errors
    /// Returns why the value has no literal source.
    pub fn laid_out(&self, indent: &str, column: usize) -> Result<String, Unwritable> {
        Item::Value(self).laid_out(indent, column)
    }
}

/// Vun source being written.
#[derive(Default)]
pub(crate) struct Writer {
    pub(crate) out: String,
}

impl Writer {
    /// How punctuation of `kind` is spelled.
    pub(crate) fn spelling(kind: TokenKind) -> &'static str {
        kind.text().expect("punctuation has a spelling")
    }

    pub(crate) fn token(&mut self, kind: TokenKind) -> &mut Self {
        self.out.push_str(Self::spelling(kind));
        self
    }

    pub(crate) fn text(&mut self, text: &str) -> &mut Self {
        self.out.push_str(text);
        self
    }

    /// A line break, then `indent`.
    pub(crate) fn line(&mut self, indent: &str) -> &mut Self {
        self.out.push('\n');
        self.text(indent)
    }

    /// `name`, escaped when it is a keyword.
    pub(crate) fn binder(&mut self, name: &str) -> Result<&mut Self, Unwritable> {
        if !is_ident(name) {
            return Err(Unwritable::Name);
        }
        if TokenKind::keyword(name).is_some() {
            self.out.push(ESCAPE);
        }
        Ok(self.text(name))
    }

    /// `.name`
    fn name(&mut self, name: &str) -> Result<&mut Self, Unwritable> {
        self.token(TokenKind::Dot).binder(name)
    }

    /// `.name = `, before a field's value.
    pub(super) fn field(&mut self, name: &str) -> Result<&mut Self, Unwritable> {
        Ok(self.name(name)?.text(" ").token(TokenKind::Eq).text(" "))
    }

    pub(crate) fn value(&mut self, value: &Literal<'_>) -> Result<(), Unwritable> {
        match value {
            Literal::Name(name) => self.name(name).map(drop),
            Literal::Tagged(tag, payload) => {
                self.name(tag)?.token(TokenKind::LParen);
                self.items(payload, Self::value)?;
                self.token(TokenKind::RParen);
                Ok(())
            }
            Literal::String(text) => {
                self.string(text);
                Ok(())
            }
            Literal::Int(value) => {
                let _ = write!(self.out, "{value}");
                Ok(())
            }
            Literal::List(items) => self.braced(TokenKind::DotLBrace, items, Self::value),
            Literal::Record(fields) => self.record(TokenKind::DotLBrace, fields),
            Literal::Typed(path, record) => match &**record {
                Literal::Record(fields) => self.path(path)?.record(TokenKind::LBrace, fields),
                _ => Err(Unwritable::Opaque),
            },
            Literal::Call(path, args) => {
                self.path(path)?.token(TokenKind::LParen);
                self.items(args, Self::value)?;
                self.token(TokenKind::RParen);
                Ok(())
            }
            Literal::Hole => {
                self.token(TokenKind::Underscore);
                Ok(())
            }
            Literal::Enum(_) | Literal::Namespace(_) => Err(Unwritable::Type),
            Literal::Opaque => Err(Unwritable::Opaque),
        }
    }

    /// `a.b.c`
    pub(crate) fn path(&mut self, path: &[&str]) -> Result<&mut Self, Unwritable> {
        for (i, name) in path.iter().enumerate() {
            if i > 0 {
                self.token(TokenKind::Dot);
            }
            self.binder(name)?;
        }
        Ok(self)
    }

    /// `.{ .a = x }` after `open`.
    fn record(&mut self, open: TokenKind, fields: &[Decl<'_>]) -> Result<(), Unwritable> {
        self.braced(open, fields, |w, d| w.field(d.name)?.value(&d.value))
    }

    /// `.{ a, b }`, or `.{}` when empty, opened by `open`.
    fn braced<T>(
        &mut self,
        open: TokenKind,
        items: &[T],
        item: impl Fn(&mut Self, &T) -> Result<(), Unwritable>,
    ) -> Result<(), Unwritable> {
        self.token(open);
        if !items.is_empty() {
            self.text(" ");
            self.items(items, item)?;
            self.text(" ");
        }
        self.token(TokenKind::RBrace);
        Ok(())
    }

    /// `a, b`
    pub(crate) fn items<T>(
        &mut self,
        items: &[T],
        item: impl Fn(&mut Self, &T) -> Result<(), Unwritable>,
    ) -> Result<(), Unwritable> {
        for (i, it) in items.iter().enumerate() {
            if i > 0 {
                self.token(TokenKind::Comma).text(" ");
            }
            item(self, it)?;
        }
        Ok(())
    }

    /// A quoted string whose escapes read back as `text`.
    pub(crate) fn string(&mut self, text: &str) {
        const QUOTE: char = '"';
        self.out.push(QUOTE);
        for c in text.chars() {
            match SIMPLE_ESCAPES.iter().find(|&&(_, value)| value == c && c != '\'') {
                Some(&(escape, _)) => {
                    self.out.push(ESCAPE);
                    self.out.push(escape);
                }
                None if c.is_control() => {
                    let _ = write!(self.out, "{ESCAPE}u{{{:x}}}", u32::from(c));
                }
                None => self.out.push(c),
            }
        }
        self.out.push(QUOTE);
    }
}
