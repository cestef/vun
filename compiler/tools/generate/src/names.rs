//! Rust spellings of names from Vun sources.

use proc_macro2::{Ident, Span};

pub(crate) fn is_type_name(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_uppercase())
}

/// Doc lines as `#[doc]` values, which carry the space after `///`.
pub(crate) fn docs(lines: &[&str]) -> Vec<String> {
    lines.iter().map(|l| format!(" {l}")).collect()
}

pub(crate) fn ident(name: &str) -> Ident {
    Ident::new(name, Span::call_site())
}

/// A module named `name`, raw when the name is a Rust keyword: the group `impl` is `r#impl`.
pub(crate) fn module(name: &str) -> Ident {
    match syn::parse_str::<Ident>(name) {
        Ok(ident) => ident,
        Err(_) => Ident::new_raw(name, Span::call_site()),
    }
}

/// The Rust spelling of a `snake_case` grammar name: `colon_eq` is `ColonEq`.
pub(crate) fn pascal(name: &str) -> Ident {
    let word: String = name
        .split('_')
        .flat_map(|part| {
            let mut chars = part.chars();
            chars.next().map(|c| c.to_ascii_uppercase()).into_iter().chain(chars)
        })
        .collect();
    ident(&word)
}

/// The Rust constant for a table: `element_types` is `ELEMENT_TYPES`.
pub(crate) fn constant(name: &str) -> Ident {
    ident(&name.to_ascii_uppercase())
}

/// The module for a type name: `TokenKind` is `token_kind`.
pub(crate) fn snake(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}
