//! Writing literal values as Overt source, the inverse of reading them, so that tools change
//! manifests through typed values rather than formatted text.

use std::fmt::{self, Write};

use overt_grammar::TokenKind;

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

/// The width lines are kept within, and the indentation of each nesting level, until `overt fmt`
/// owns the layout.
pub const WIDTH: usize = 100;
pub(super) const INDENT: &str = "    ";

pub(super) fn token(kind: TokenKind) -> &'static str {
    kind.text().expect("punctuation has a spelling")
}

impl Literal<'_> {
    /// The source that reads back as this value, on one line.
    ///
    /// # Errors
    /// Returns why the value has no literal source.
    pub fn source(&self) -> Result<String, Unwritable> {
        let mut out = String::new();
        self.write(&mut out)?;
        Ok(out)
    }

    /// The source of this value starting `column` characters into a line indented by `indent`:
    /// one line when it fits within [`WIDTH`], otherwise lists and records hold one item per line.
    ///
    /// # Errors
    /// Returns why the value has no literal source.
    pub fn laid_out(&self, indent: &str, column: usize) -> Result<String, Unwritable> {
        let flat = self.source()?;
        let items: Vec<Item<'_, '_>> = match self {
            Self::List(items) if !items.is_empty() => items.iter().map(Item::Value).collect(),
            Self::Record(fields) if !fields.is_empty() => fields.iter().map(Item::Field).collect(),
            _ => return Ok(flat),
        };
        if column + flat.len() <= WIDTH {
            return Ok(flat);
        }
        let inner = format!("{indent}{INDENT}");
        let mut out = token(TokenKind::DotLBrace).to_owned();
        for item in items {
            out.push('\n');
            out.push_str(&inner);
            out.push_str(&item.laid_out(&inner)?);
            out.push_str(token(TokenKind::Comma));
        }
        out.push('\n');
        out.push_str(indent);
        out.push_str(token(TokenKind::RBrace));
        Ok(out)
    }

    fn write(&self, out: &mut String) -> Result<(), Unwritable> {
        match self {
            Self::Name(name) => write_field_name(name, out),
            Self::Tagged(tag, payload) => {
                write_field_name(tag, out)?;
                out.push_str(token(TokenKind::LParen));
                write_items(payload, out, Self::write)?;
                out.push_str(token(TokenKind::RParen));
                Ok(())
            }
            Self::String(text) => {
                write_string(text, out);
                Ok(())
            }
            Self::Int(value) => {
                let _ = write!(out, "{value}");
                Ok(())
            }
            Self::List(items) => write_braced(items, out, Self::write),
            Self::Record(fields) => write_braced(fields, out, |d, out| write_field(d.name, &d.value, out)),
            Self::Enum(_) | Self::Namespace(_) => Err(Unwritable::Type),
            Self::Opaque => Err(Unwritable::Opaque),
        }
    }
}

/// An item of a list or record laid out on its own line.
enum Item<'a, 's> {
    Value(&'a Literal<'s>),
    Field(&'a Decl<'s>),
}

impl Item<'_, '_> {
    fn laid_out(&self, indent: &str) -> Result<String, Unwritable> {
        // The comma after the item counts toward the width.
        let column = indent.len() + token(TokenKind::Comma).len();
        match self {
            Self::Value(value) => value.laid_out(indent, column),
            Self::Field(decl) => laid_out_field(decl.name, &decl.value, indent, column),
        }
    }
}

/// `.name = value`, laid out from `column`.
pub(super) fn laid_out_field(
    name: &str,
    value: &Literal<'_>,
    indent: &str,
    column: usize,
) -> Result<String, Unwritable> {
    let mut out = String::new();
    write_field_prefix(name, &mut out)?;
    let value = value.laid_out(indent, column + out.len())?;
    out.push_str(&value);
    Ok(out)
}

fn write_field(name: &str, value: &Literal<'_>, out: &mut String) -> Result<(), Unwritable> {
    write_field_prefix(name, out)?;
    value.write(out)
}

/// `.name = `
pub(super) fn write_field_prefix(name: &str, out: &mut String) -> Result<(), Unwritable> {
    write_field_name(name, out)?;
    out.push(' ');
    out.push_str(token(TokenKind::Eq));
    out.push(' ');
    Ok(())
}

/// `.name`, escaped when the name is a keyword.
fn write_field_name(name: &str, out: &mut String) -> Result<(), Unwritable> {
    if !is_ident(name) {
        return Err(Unwritable::Name);
    }
    out.push_str(token(TokenKind::Dot));
    if TokenKind::keyword(name).is_some() {
        out.push(ESCAPE);
    }
    out.push_str(name);
    Ok(())
}

/// `.{ a, b }`, or `.{}` when empty.
fn write_braced<T>(
    items: &[T],
    out: &mut String,
    write: impl Fn(&T, &mut String) -> Result<(), Unwritable>,
) -> Result<(), Unwritable> {
    out.push_str(token(TokenKind::DotLBrace));
    if !items.is_empty() {
        out.push(' ');
        write_items(items, out, write)?;
        out.push(' ');
    }
    out.push_str(token(TokenKind::RBrace));
    Ok(())
}

fn write_items<T>(
    items: &[T],
    out: &mut String,
    write: impl Fn(&T, &mut String) -> Result<(), Unwritable>,
) -> Result<(), Unwritable> {
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            out.push_str(token(TokenKind::Comma));
            out.push(' ');
        }
        write(item, out)?;
    }
    Ok(())
}

/// A quoted string whose escapes read back as `text`.
fn write_string(text: &str, out: &mut String) {
    const QUOTE: char = '"';
    out.push(QUOTE);
    for c in text.chars() {
        match SIMPLE_ESCAPES.iter().find(|&&(_, value)| value == c && c != '\'') {
            Some(&(escape, _)) => {
                out.push(ESCAPE);
                out.push(escape);
            }
            None if c.is_control() => {
                let _ = write!(out, "{ESCAPE}u{{{:x}}}", u32::from(c));
            }
            None => out.push(c),
        }
    }
    out.push(QUOTE);
}
