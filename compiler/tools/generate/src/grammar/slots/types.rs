//! The generated types of a kind with productions, `NodeField`, `NodeSlot` and `NodeTerm`, and the
//! values of its productions and slots.

use proc_macro2::TokenStream;
use quote::quote;

use super::Types;
use crate::grammar::Enum;
use crate::grammar::model::{Restriction, Slot, Term};
use crate::names::{constant, ident, pascal};

impl Types<'_> {
    pub(super) fn field(&self, kind: &Enum<'_>) -> TokenStream {
        let (field_ty, fields) = (&self.field, &self.model.fields);
        let variants: Vec<_> = fields.iter().map(|f| pascal(f)).collect();
        let doc = format!(" The named children of [`{}`] kinds.", kind.name);
        quote! {
            #[doc = #doc]
            #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, vun_codec::Encode, vun_codec::Decode)]
            pub enum #field_ty { #(#variants),* }

            impl #field_ty {
                /// The name of this field in `nodes.vun`.
                #[must_use]
                pub const fn name(self) -> &'static str {
                    match self {
                        #(Self::#variants => #fields,)*
                    }
                }
            }
        }
    }

    pub(super) fn slot(&self, kind: &Enum<'_>) -> TokenStream {
        let (slot_ty, field_ty, count_ty, kind_ty) = (&self.slot, &self.field, &self.count, &self.kind);
        let doc = format!(
            " One child position of a [`{}`]: its field, how many nodes it holds, and the kind they must have, if restricted.",
            kind.name
        );
        quote! {
            #[doc = #doc]
            #[derive(Clone, Copy, Debug, PartialEq, Eq)]
            pub struct #slot_ty {
                pub field: #field_ty,
                pub count: #count_ty,
                pub kind: Option<#kind_ty>,
            }

            impl #slot_ty {
                #[must_use]
                pub const fn new(field: #field_ty, count: #count_ty, kind: Option<#kind_ty>) -> Self {
                    Self { field, count, kind }
                }
            }
        }
    }

    pub(super) fn term(&self, kind: &Enum<'_>) -> TokenStream {
        let (term_ty, field_ty, kind_ty, tokens) = (&self.term, &self.field, &self.kind, &self.tokens);
        let doc = format!(" A term of the production of a [`{}`] (see `nodes.vun`).", kind.name);
        quote! {
            #[doc = #doc]
            #[derive(Clone, Copy, Debug, PartialEq, Eq)]
            pub enum #term_ty {
                /// A token of one of these kinds.
                Token(&'static [#tokens]),
                /// A name, or a token spelled, as one of these words.
                Word(&'static [&'static str]),
                /// A child in this field.
                Field(#field_ty),
                /// A child in this field, of one of these kinds.
                Of(#field_ty, &'static [#kind_ty]),
                /// A child in this field, of one of these kinds or a transparent kind ending with
                /// one.
                Ending(#field_ty, &'static [#kind_ty]),
                /// A child in this field starting with a token of one of these kinds.
                Starting(#field_ty, &'static [#tokens]),
                /// These terms in sequence, or nothing.
                Optional(&'static [#term_ty]),
                /// These terms in sequence, any number of times.
                Many(&'static [#term_ty]),
                /// Any number of the first term, with the second between them and after the last
                /// if written.
                Separated(&'static #term_ty, &'static #term_ty),
                /// One of these terms.
                Choice(&'static [#term_ty]),
                /// These terms in sequence.
                Sequence(&'static [#term_ty]),
            }
        }
    }

    /// `terms` as a Rust slice of terms.
    pub(super) fn slice(&self, terms: &[Term]) -> TokenStream {
        let terms = terms.iter().map(|t| self.value(t));
        quote! { &[#(#terms),*] }
    }

    pub(super) fn value(&self, term: &Term) -> TokenStream {
        let (ty, field_ty) = (&self.term, &self.field);
        let field = |f| pascal(self.model.field_name(f));
        match term {
            Term::Tokens(tokens) => {
                let (kind, names) = (&self.tokens, tokens.iter().map(|&t| pascal(self.model.token_name(t))));
                quote! { #ty::Token(&[#(#kind::#names),*]) }
            }
            Term::Words(words) => quote! { #ty::Word(&[#(#words),*]) },
            Term::Field(f, Some(restriction)) => {
                let variant = field(*f);
                let (form, table) = match restriction {
                    Restriction::Of(table) => (ident("Of"), constant(&table.name)),
                    Restriction::Ending(table) => (ident("Ending"), constant(&table.name)),
                    Restriction::Starting(table) => (ident("Starting"), constant(&table.name)),
                };
                quote! { #ty::#form(#field_ty::#variant, #table) }
            }
            Term::Field(f, None) => {
                let variant = field(*f);
                quote! { #ty::Field(#field_ty::#variant) }
            }
            Term::Optional(terms) => self.compound("Optional", terms),
            Term::Many(terms) => self.compound("Many", terms),
            Term::Choice(terms) => self.compound("Choice", terms),
            Term::Sequence(terms) => self.compound("Sequence", terms),
            Term::Separated(item, separator) => {
                let (item, separator) = (self.value(item), self.value(separator));
                quote! { #ty::Separated(&#item, &#separator) }
            }
        }
    }

    pub(super) fn compound(&self, variant: &str, terms: &[Term]) -> TokenStream {
        let (ty, variant, terms) = (&self.term, ident(variant), self.slice(terms));
        quote! { #ty::#variant(#terms) }
    }

    pub(super) fn slot_value(&self, s: Slot) -> TokenStream {
        let (slot_ty, field_ty, count_ty, kind_ty) = (&self.slot, &self.field, &self.count, &self.kind);
        let (field, count) = (pascal(self.model.field_name(s.field)), pascal(s.count.name()));
        let node = self.model.named_kind(s.field).map_or_else(
            || quote! { None },
            |kind| {
                let kind = pascal(self.model.kind_name(kind));
                quote! { Some(#kind_ty::#kind) }
            },
        );
        quote! { #slot_ty::new(#field_ty::#field, #count_ty::#count, #node) }
    }
}
