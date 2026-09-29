//! Kinds and enums, emitted as Rust enums with their lists and spellings.

use proc_macro2::TokenStream;
use quote::quote;

use super::Enum;
use crate::names::{constant, ident, pascal};

impl Enum<'_> {
    /// The enum with its lists, and apart, as the items of a module of their own, the name and
    /// spelling of each variant.
    pub(super) fn emit(&self) -> (TokenStream, TokenStream) {
        let name = ident(&self.name);
        let doc = &self.doc;
        let variants = self.variants.iter().map(|v| {
            let (ident, doc) = (pascal(&v.name), &v.doc);
            quote! { #(#[doc = #doc])* #ident }
        });
        let lists = self.lists.iter().map(|(list, members)| {
            let (list, members) = (constant(list), members.iter().map(|m| pascal(m)));
            quote! { pub const #list: &[Self] = &[#(Self::#members),*]; }
        });
        let every = self.variants.iter().map(|v| pascal(&v.name));
        let lists = quote! {
            impl #name {
                /// Every variant, in the order declared.
                pub const ALL: &[Self] = &[#(Self::#every),*];
                #(#lists)*
            }
        };
        let spelled: Vec<_> =
            self.variants.iter().filter_map(|v| Some((pascal(&v.name), v.text.as_deref()?))).collect();
        let unspelled = (spelled.len() < self.variants.len()).then(|| quote! { _ => None, });
        let text = (!spelled.is_empty()).then(|| {
            let (variants, texts): (Vec<_>, Vec<_>) = spelled.into_iter().unzip();
            quote! {
                /// The fixed spelling of this kind, if it has one.
                #[must_use]
                pub const fn text(self) -> Option<&'static str> {
                    match self {
                        #(Self::#variants => Some(#texts),)*
                        #unspelled
                    }
                }
            }
        });
        let (idents, names): (Vec<_>, Vec<_>) =
            self.variants.iter().map(|v| (pascal(&v.name), v.name.as_ref())).unzip();
        let enum_items = quote! {
            #(#[doc = #doc])*
            #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, vun_codec::Encode, vun_codec::Decode)]
            pub enum #name { #(#variants),* }
            #lists
        };
        let names = quote! {
            impl #name {
                /// Its name in the grammar.
                #[must_use]
                #[allow(clippy::too_many_lines, reason = "one arm per variant, as declared")]
                pub const fn name(self) -> &'static str {
                    match self {
                        #(Self::#idents => #names,)*
                    }
                }
                #text
            }
        };
        (enum_items, names)
    }
}
