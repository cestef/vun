//! Lua table constructors, written: an object is a table of named fields, an array a list, so a
//! configuration a Lua program reads (`return { ... }`) is one typed value too. Lua has no null:
//! a null field is left out, as an absent option is.

use std::fmt::{self, Write};

use super::json::Json;
use crate::Value;

/// A value written as a Lua expression by [`fmt::Display`]; `{:#}` puts each field on its own
/// line.
#[derive(Clone, Copy, Debug)]
pub struct Lua<'v, 'a>(pub &'v Value<'a>);

/// The indentation of one level in the alternate form.
const INDENT: &str = "  ";

impl fmt::Display for Lua<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let indent = f.alternate().then_some(INDENT);
        write(f, self.0, indent, 0)
    }
}

fn write(f: &mut dyn Write, value: &Value<'_>, indent: Option<&str>, depth: usize) -> fmt::Result {
    let (entries, named): (Vec<(Option<&str>, &Value<'_>)>, bool) = match value {
        Value::Null => return f.write_str("nil"),
        Value::Array(items) => (items.iter().map(|i| (None, i)).collect(), false),
        Value::Object(object) => {
            (object.iter().filter(|(_, v)| !v.is_null()).map(|(n, v)| (Some(n), v)).collect(), true)
        }
        // Strings, numbers and booleans are written as JSON writes them.
        scalar => return write!(f, "{}", Json(scalar)),
    };
    if entries.is_empty() {
        return f.write_str("{}");
    }
    f.write_char('{')?;
    for (i, (name, value)) in entries.into_iter().enumerate() {
        f.write_str(if i > 0 { "," } else { "" })?;
        match indent {
            Some(unit) => {
                f.write_char('\n')?;
                (0..=depth).try_for_each(|_| f.write_str(unit))?;
            }
            None => f.write_char(' ')?,
        }
        if let Some(name) = name.filter(|_| named) {
            key(f, name)?;
            f.write_str(" = ")?;
        }
        write(f, value, indent, depth + 1)?;
    }
    match indent {
        Some(unit) => {
            f.write_str(",\n")?;
            (0..depth).try_for_each(|_| f.write_str(unit))?;
            f.write_char('}')
        }
        None => f.write_str(" }"),
    }
}

/// A field name, bare where it is a Lua name, bracketed as a string elsewhere.
fn key(f: &mut dyn Write, name: &str) -> fmt::Result {
    let bare = name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        && !KEYWORDS.contains(&name);
    if bare { f.write_str(name) } else { write!(f, "[{}]", Json(&Value::from(name.to_owned()))) }
}

/// Lua's reserved words, which a bare field name cannot be.
const KEYWORDS: [&str; 22] = [
    "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "goto", "if", "in", "local",
    "nil", "not", "or", "repeat", "return", "then", "true", "until", "while",
];

#[cfg(test)]
mod tests;
