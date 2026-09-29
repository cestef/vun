//! `#[derive(FromLiteral)]` for `vun_syntax::literal`. The item's shape decides the literal it
//! reads:
//!
//! - a struct of named fields is a record `.{ .field = value }`, a field read by its name, an
//!   option left out read as `None`;
//! - a struct of one unnamed field is that field;
//! - an enum's unit variant is `.variant`, and a variant of unnamed fields `.variant(a, b)`, a
//!   value per field; a last field marked `#[literal(rest)]` reads the values left as a list.
//!   A variant of one field marked `#[literal(untagged)]` is any value that field reads, tried
//!   after the tagged forms, in order.
//!
//! Names are the item's own, variants in `snake_case`; `#[literal(name = "...")]` names one field
//! or variant as the literal writes it.

mod options;
mod read;

use proc_macro::TokenStream;
use vun_derive_core::Item;

use options::Literal;

#[proc_macro_derive(FromLiteral, attributes(literal))]
pub fn from_literal(input: TokenStream) -> TokenStream {
    let item = match Item::parse(input) {
        Ok(item) => item,
        Err(unsupported) => return unsupported.report(),
    };
    Literal::of(&item).map_or_else(vun_derive_core::Unsupported::report, |literal| literal.read())
}
