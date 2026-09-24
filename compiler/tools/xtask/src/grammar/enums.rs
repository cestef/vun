//! Kinds and enums, emitted as Rust enums with their lists and spellings.

use std::borrow::Cow;

use overt_syntax::literal::Literal;
use proc_macro2::TokenStream;
use quote::quote;

use super::{ENTRY_NAME, ENTRY_TEXT, Enum, Slot, Variant};
use crate::names::{constant, ident, pascal};

impl Enum<'_> {
    pub(super) fn emit(&self) -> TokenStream {
        let name = ident(self.name);
        let doc = &self.doc;
        let variants = self.variants.iter().map(|v| {
            let (ident, doc) = (pascal(v.name), &v.doc);
            quote! { #(#[doc = #doc])* #ident }
        });
        let lists = self.lists.iter().map(|(list, members)| {
            let (list, members) = (constant(list), members.iter().map(|m| pascal(m)));
            quote! { pub const #list: &[Self] = &[#(Self::#members),*]; }
        });
        let spelled: Vec<_> = self.variants.iter().filter_map(|v| Some((pascal(v.name), v.text?))).collect();
        let text = (!spelled.is_empty()).then(|| {
            let (variants, texts): (Vec<_>, Vec<_>) = spelled.into_iter().unzip();
            quote! {
                /// The fixed spelling of this kind, if it has one.
                #[must_use]
                pub const fn text(self) -> Option<&'static str> {
                    match self {
                        #(Self::#variants => Some(#texts),)*
                        _ => None,
                    }
                }
            }
        });
        let body = (!self.lists.is_empty()).then(|| quote! { impl #name { #(#lists)* #text } });
        quote! {
            #(#[doc = #doc])*
            #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
            pub enum #name { #(#variants),* }
            #body
        }
    }
}

impl<'s> Variant<'s> {
    /// A kind's entry: `.name`, a spelling `"text"` named after itself, `.{ .name, .text }`, or
    /// `.name(.count(.field), ...)` with the kind's children.
    pub(super) fn entry(entry: &Literal<'s>) -> Option<Self> {
        let (name, text) = match entry {
            Literal::Name(name) => (*name, None),
            Literal::Tagged(name, payload) => {
                let slots = payload.iter().map(Slot::of).collect::<Option<_>>()?;
                return Some(Self { name, doc: Vec::new(), text: None, slots });
            }
            Literal::String(Cow::Borrowed(text)) => (*text, Some(*text)),
            Literal::Record(_) => match (entry.get(ENTRY_NAME)?, entry.get(ENTRY_TEXT)?) {
                (Literal::Name(name), Literal::String(Cow::Borrowed(text))) => (*name, Some(*text)),
                _ => return None,
            },
            _ => return None,
        };
        Some(Self { name, doc: Vec::new(), text, slots: Vec::new() })
    }
}

impl<'s> Slot<'s> {
    /// `.count(.field)`.
    fn of(value: &Literal<'s>) -> Option<Self> {
        let (count, [Literal::Name(field)]) = value.as_tagged()? else { return None };
        Some(Self { count, field })
    }
}
