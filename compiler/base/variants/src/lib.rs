//! `#[derive(Variants)]` on an enum of unit variants: `pub const ALL: &[Self]`, every variant in
//! declaration order, so that a table over an enum is never written out a second time.

use proc_macro::{Delimiter, TokenStream};
use vun_derive_core::{Body, Fields, Item, Tokens, Unsupported};

#[proc_macro_derive(Variants)]
pub fn variants(input: TokenStream) -> TokenStream {
    let item = match Item::try_from(input) {
        Ok(item) => item,
        Err(unsupported) => return unsupported.report(),
    };
    let Body::Enum(variants) = &item.body else { return Unsupported("`Variants` is for enums").report() };
    if !item.generics.params.is_empty() || variants.iter().any(|v| !matches!(v.fields, Fields::Unit)) {
        return Unsupported("`Variants` is for enums of unit variants, without generics").report();
    }
    let all = Tokens::new().comma(item.cases(), |t, case| t.then(case.path));
    let constant = Tokens::new()
        .word("pub")
        .word("const")
        .word("ALL")
        .op(":")
        .op("&")
        .group(Delimiter::Bracket, Tokens::new().word("Self"))
        .op("=")
        .op("&")
        .group(Delimiter::Bracket, all)
        .op(";");
    Tokens::new()
        .attr(Tokens::new().word("automatically_derived"))
        .word("impl")
        .tree(item.name.clone())
        .braces(constant)
        .into()
}
