//! `#[derive(ToData, FromData)]` for `vun-data`. The item's shape decides its data:
//!
//! - a struct of named fields is an object, a member per field, an absent option left out; a
//!   field marked `#[data(flatten)]` adds its own members instead;
//! - a struct of one unnamed field is that field, of several an array, of none `null`;
//! - an enum of unit variants is a string naming the variant, or, marked `#[data(number)]`, its
//!   discriminant; an enum marked `#[data(untagged)]` is its variant's value, read as the first
//!   variant it fits;
//! - any other enum is tagged: a unit variant is a string naming it, another an object whose one
//!   member, named by the variant, holds its value.
//!
//! Names are the item's own, cased by `#[data(case = camel)]` on the item (`snake` otherwise);
//! `#[data(name = "...")]` names one field or variant on the wire.

mod options;
mod read;
mod tokens;
mod write;

use proc_macro::TokenStream;
use vun_derive_core::{Item, Unsupported};

use options::Data;

// A method of `Data<'_>` names one lifetime of it, and an expansion takes any.
#[expect(clippy::redundant_closure_for_method_calls, reason = "the method is not general enough")]
#[proc_macro_derive(ToData, attributes(data))]
pub fn to_data(input: TokenStream) -> TokenStream {
    derive(input, |data| data.write())
}

#[expect(clippy::redundant_closure_for_method_calls, reason = "the method is not general enough")]
#[proc_macro_derive(FromData, attributes(data))]
pub fn from_data(input: TokenStream) -> TokenStream {
    derive(input, |data| data.read())
}

fn derive(input: TokenStream, expand: fn(&Data<'_>) -> TokenStream) -> TokenStream {
    let item = match Item::parse(input) {
        Ok(item) => item,
        Err(unsupported) => return unsupported.report(),
    };
    Data::of(&item).map_or_else(Unsupported::report, |data| expand(&data))
}
