//! TOML, written: an object is a document, so a configuration is one typed value
//! (`#[derive(ToData)]`) written as JSON or as TOML.
//!
//! Members holding plain values come first, as `key = value`; objects follow as tables
//! (`[a.b]`), and arrays of objects as arrays of tables (`[[a]]`). An object inside an array of
//! other values is an inline table. TOML has no null: a null member is left out, as an absent
//! option is.

use std::fmt::{self, Write};

use super::json::Json;
use crate::{Object, Value};

/// A TOML document: the object `root`, written as TOML by [`fmt::Display`].
#[derive(Clone, Copy, Debug)]
pub struct Toml<'v, 'a>(pub &'v Object<'a>);

impl fmt::Display for Toml<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        table(f, self.0, &mut Vec::new())
    }
}

/// A value written as TOML on one line, as a key's value: an object is an inline table.
#[derive(Clone, Copy, Debug)]
pub struct Inline<'v, 'a>(pub &'v Value<'a>);

impl fmt::Display for Inline<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        inline(f, self.0)
    }
}

/// Whether a member is written below the plain ones, under a header of its own.
fn sectioned(value: &Value<'_>) -> bool {
    match value {
        Value::Object(_) => true,
        Value::Array(items) => !items.is_empty() && items.iter().all(|i| matches!(i, Value::Object(_))),
        _ => false,
    }
}

/// The members of `object`, whose path from the root is `path`.
fn table<'k>(f: &mut fmt::Formatter<'_>, object: &'k Object<'_>, path: &mut Vec<&'k str>) -> fmt::Result {
    for (name, value) in object.iter().filter(|(_, v)| !sectioned(v) && !v.is_null()) {
        key(f, name)?;
        f.write_str(" = ")?;
        inline(f, value)?;
        f.write_char('\n')?;
    }
    for (name, value) in object.iter().filter(|(_, v)| sectioned(v)) {
        path.push(name);
        match value {
            Value::Object(inner) => {
                // A table holding only tables is implied by theirs.
                if inner.is_empty() || inner.iter().any(|(_, v)| !sectioned(v) && !v.is_null()) {
                    header(f, path, "[", "]")?;
                }
                table(f, inner, path)?;
            }
            Value::Array(items) => {
                for item in items.iter().filter_map(Value::as_object) {
                    header(f, path, "[[", "]]")?;
                    table(f, item, path)?;
                }
            }
            _ => {}
        }
        path.pop();
    }
    Ok(())
}

fn header(f: &mut fmt::Formatter<'_>, path: &[&str], open: &str, close: &str) -> fmt::Result {
    write!(f, "\n{open}")?;
    for (i, name) in path.iter().enumerate() {
        if i > 0 {
            f.write_char('.')?;
        }
        key(f, name)?;
    }
    writeln!(f, "{close}")
}

/// A key, bare where TOML allows it, quoted elsewhere.
fn key(f: &mut fmt::Formatter<'_>, name: &str) -> fmt::Result {
    let bare = !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    if bare { f.write_str(name) } else { write!(f, "{}", Json(&Value::from(name.to_owned()))) }
}

/// A value on one line: JSON's strings and numbers are TOML's, objects inline tables.
fn inline(f: &mut fmt::Formatter<'_>, value: &Value<'_>) -> fmt::Result {
    match value {
        Value::Array(items) => {
            f.write_char('[')?;
            for (i, item) in items.iter().filter(|i| !i.is_null()).enumerate() {
                if i > 0 {
                    f.write_str(", ")?;
                }
                inline(f, item)?;
            }
            f.write_char(']')
        }
        Value::Object(object) => {
            f.write_char('{')?;
            for (i, (name, value)) in object.iter().filter(|(_, v)| !v.is_null()).enumerate() {
                f.write_str(if i > 0 { ", " } else { " " })?;
                key(f, name)?;
                f.write_str(" = ")?;
                inline(f, value)?;
            }
            f.write_str(if object.is_empty() { "}" } else { " }" })
        }
        scalar => write!(f, "{}", Json(scalar)),
    }
}

#[cfg(test)]
mod tests;
