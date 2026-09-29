//! `#[derive(ToJson, FromJson)]` for `vun-json`. The item's shape decides its JSON:
//!
//! - a struct of named fields is an object, a member per field, an absent option left out; a
//!   field marked `#[json(flatten)]` adds its own members instead;
//! - a struct of one unnamed field is that field, of several an array, of none `null`;
//! - an enum of unit variants is a string naming the variant, or, marked `#[json(number)]`, its
//!   discriminant; an enum marked `#[json(untagged)]` is its variant's value, read as the first
//!   variant it fits.
//!
//! Names are the item's own, cased by `#[json(case = camel)]` on the item (`snake` otherwise);
//! `#[json(name = "...")]` names one field or variant on the wire.

mod casing;
mod options;
mod read;
mod tokens;
mod write;

use proc_macro::TokenStream;
use vun_derive_core::{Item, Unsupported};

use options::Json;

// A method of `Json<'_>` names one lifetime of it, and an expansion takes any.
#[expect(clippy::redundant_closure_for_method_calls, reason = "the method is not general enough")]
#[proc_macro_derive(ToJson, attributes(json))]
pub fn to_json(input: TokenStream) -> TokenStream {
    derive(input, |json| json.write())
}

#[expect(clippy::redundant_closure_for_method_calls, reason = "the method is not general enough")]
#[proc_macro_derive(FromJson, attributes(json))]
pub fn from_json(input: TokenStream) -> TokenStream {
    derive(input, |json| json.read())
}

fn derive(input: TokenStream, expand: fn(&Json<'_>) -> TokenStream) -> TokenStream {
    let item = match Item::parse(input) {
        Ok(item) => item,
        Err(unsupported) => return unsupported.report(),
    };
    Json::of(&item).map_or_else(Unsupported::report, |json| expand(&json))
}
