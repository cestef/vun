//! Rust spellings of grammar names.

use proc_macro2::{Ident, Span};

pub(super) fn is_type_name(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_uppercase())
}

/// Doc lines as `#[doc]` values, which carry the space after `///`.
pub(super) fn docs(lines: &[&str]) -> Vec<String> {
    lines.iter().map(|l| format!(" {l}")).collect()
}

pub(super) fn ident(name: &str) -> Ident {
    Ident::new(name, Span::call_site())
}

/// The Rust spelling of a `snake_case` grammar name: `colon_eq` is `ColonEq`.
pub(super) fn pascal(name: &str) -> Ident {
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
pub(super) fn constant(name: &str) -> Ident {
    ident(&name.to_ascii_uppercase())
}
