//! Writing JSON: `{}` writes a value compactly, `{:#}` with each member and item on its own line,
//! indented by two spaces. Strings escape exactly what the grammar requires: the quote, the
//! backslash and control characters.

use std::fmt::{self, Write};

use crate::{Object, Value};

/// The indentation of one level in the alternate form.
const INDENT: &str = "  ";

/// A value or object, written as JSON by [`fmt::Display`]: `Json(&value).to_string()`.
#[derive(Clone, Copy, Debug)]
pub struct Json<T>(pub T);

impl fmt::Display for Json<&Value<'_>> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let indent = if f.alternate() { Some(INDENT) } else { None };
        self.0.write(f, indent, 0)
    }
}

impl fmt::Display for Json<&Object<'_>> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let indent = if f.alternate() { Some(INDENT) } else { None };
        write_object(self.0, f, indent, 0)
    }
}

impl Value<'_> {
    fn write(&self, f: &mut dyn Write, indent: Option<&str>, depth: usize) -> fmt::Result {
        match self {
            Self::Null => f.write_str("null"),
            Self::Bool(b) => f.write_str(if *b { "true" } else { "false" }),
            Self::Number(n) => f.write_str(n.as_str()),
            Self::String(s) => write_string(f, s),
            Self::Array(items) if items.is_empty() => f.write_str("[]"),
            Self::Array(items) => {
                f.write_char('[')?;
                for (i, item) in items.iter().enumerate() {
                    separate(f, i, indent, depth + 1)?;
                    item.write(f, indent, depth + 1)?;
                }
                line(f, indent, depth)?;
                f.write_char(']')
            }
            Self::Object(o) => write_object(o, f, indent, depth),
        }
    }
}

fn write_object(o: &Object<'_>, f: &mut dyn Write, indent: Option<&str>, depth: usize) -> fmt::Result {
    if o.is_empty() {
        return f.write_str("{}");
    }
    f.write_char('{')?;
    for (i, (name, value)) in o.iter().enumerate() {
        separate(f, i, indent, depth + 1)?;
        write_string(f, name)?;
        f.write_str(if indent.is_some() { ": " } else { ":" })?;
        value.write(f, indent, depth + 1)?;
    }
    line(f, indent, depth)?;
    f.write_char('}')
}

/// The comma before every item but the first, then the item's line.
fn separate(f: &mut dyn Write, i: usize, indent: Option<&str>, depth: usize) -> fmt::Result {
    if i > 0 {
        f.write_char(',')?;
    }
    line(f, indent, depth)
}

/// A new line at `depth`, in the indented form only.
fn line(f: &mut dyn Write, indent: Option<&str>, depth: usize) -> fmt::Result {
    let Some(indent) = indent else { return Ok(()) };
    f.write_char('\n')?;
    (0..depth).try_for_each(|_| f.write_str(indent))
}

/// `s` quoted, runs of characters that need no escape written at once.
fn write_string(f: &mut dyn Write, s: &str) -> fmt::Result {
    f.write_char('"')?;
    let mut from = 0;
    for (at, byte) in s.bytes().enumerate() {
        let escape = match byte {
            b'"' => "\\\"",
            b'\\' => "\\\\",
            b'\n' => "\\n",
            b'\r' => "\\r",
            b'\t' => "\\t",
            0x08 => "\\b",
            0x0c => "\\f",
            0x00..=0x1f => "",
            _ => continue,
        };
        f.write_str(&s[from..at])?;
        if escape.is_empty() {
            write!(f, "\\u{byte:04x}")?;
        } else {
            f.write_str(escape)?;
        }
        from = at + 1;
    }
    f.write_str(&s[from..])?;
    f.write_char('"')
}
