//! `#[derive(Encode, Decode)]` for `vun-codec`: a struct encodes its fields in declaration order,
//! an enum its variant's position as a `u32`, then the variant's fields. Every type parameter is
//! bound by the derived trait.

mod expand;

use expand::Codec;
use proc_macro::TokenStream;
use vun_derive_core::{Item, Unsupported};

#[proc_macro_derive(Encode)]
pub fn encode(input: TokenStream) -> TokenStream {
    Item::try_from(input).map_or_else(Unsupported::report, |item| item.encode())
}

#[proc_macro_derive(Decode)]
pub fn decode(input: TokenStream) -> TokenStream {
    Item::try_from(input).map_or_else(Unsupported::report, |item| item.decode())
}
