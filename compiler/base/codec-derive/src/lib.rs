//! `#[derive(Encode, Decode)]` for `vun-codec`: a struct encodes its fields in declaration order,
//! an enum its variant's position as a `u32`, then the variant's fields. Every type parameter is
//! bound by the derived trait.
//!
//! Written on the compiler's `proc_macro` API alone: the item is read only as far as its name,
//! generics and field names, and the code is built as tokens.

mod expand;
mod item;
mod tokens;

use proc_macro::TokenStream;

use item::{Item, Unsupported};
use tokens::Tokens;

#[proc_macro_derive(Encode)]
pub fn encode(input: TokenStream) -> TokenStream {
    Item::parse(input).map_or_else(Unsupported::report, |item| item.encode())
}

#[proc_macro_derive(Decode)]
pub fn decode(input: TokenStream) -> TokenStream {
    Item::parse(input).map_or_else(Unsupported::report, |item| item.decode())
}

impl Unsupported {
    /// `compile_error!("why")`.
    fn report(self) -> TokenStream {
        let message = proc_macro::Literal::string(self.0);
        Tokens::new()
            .path(&["core", "compile_error"])
            .op("!")
            .parens(Tokens::new().tree(message))
            .op(";")
            .into()
    }
}
