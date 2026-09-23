//! The children of kinds whose entries are written `.name(.count(.field), ...)`.
//!
//! A kind `NodeKind` with such entries generates `NodeField`, the names of all its fields, and
//! `NodeSlot { field, count, kind }`, and `NodeKind::slots` gives each kind's slots in source
//! order. The counts are the variants of one declared enum. A field named after a variant of the
//! kind holds a node of that kind; any other field holds any node.

use proc_macro2::TokenStream;
use quote::quote;

use super::names::{ident, pascal};
use super::{Enum, Grammar};
use crate::generate::Read;

/// Suffix removed from a kind's name to name its field and slot types.
const KIND_SUFFIX: &str = "Kind";

impl Grammar<'_> {
    pub(super) fn slots(&self, kind: &Enum<'_>) -> Read<TokenStream> {
        let with_slots: Vec<_> = kind.variants.iter().filter(|v| !v.slots.is_empty()).collect();
        if with_slots.is_empty() {
            return Ok(TokenStream::new());
        }
        let counts: Vec<&str> = with_slots.iter().flat_map(|v| v.slots.iter().map(|s| s.count)).collect();
        let count_ty = ident(self.enums[self.resolve(kind.name, &counts)?].name);
        let base = kind.name.strip_suffix(KIND_SUFFIX).unwrap_or(kind.name);
        let (field_ty, slot_ty, kind_ty) =
            (ident(&format!("{base}Field")), ident(&format!("{base}Slot")), ident(kind.name));
        let mut fields: Vec<&str> = Vec::new();
        for s in with_slots.iter().flat_map(|v| &v.slots) {
            if !fields.contains(&s.field) {
                fields.push(s.field);
            }
        }
        let field_variants: Vec<_> = fields.iter().map(|f| pascal(f)).collect();
        let arms = with_slots.iter().map(|v| {
            let variant = pascal(v.name);
            let slots = v.slots.iter().map(|s| {
                let (field, count) = (pascal(s.field), pascal(s.count));
                let restricted = kind.variants.iter().any(|k| k.name == s.field);
                let node = if restricted {
                    quote! { Some(#kind_ty::#field) }
                } else {
                    quote! { None }
                };
                quote! { #slot_ty { field: #field_ty::#field, count: #count_ty::#count, kind: #node } }
            });
            quote! { Self::#variant => &[#(#slots),*], }
        });
        let doc_field = format!(" The named children of [`{}`] kinds.", kind.name);
        let doc_slot = format!(
            " One child position of a [`{}`]: its field, how many nodes it holds, and the kind they must have, if restricted.",
            kind.name
        );
        Ok(quote! {
            #[doc = #doc_field]
            #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
            pub enum #field_ty { #(#field_variants),* }

            impl #field_ty {
                /// The name of this field in `grammar.ov`.
                #[must_use]
                pub const fn name(self) -> &'static str {
                    match self {
                        #(Self::#field_variants => #fields,)*
                    }
                }
            }

            #[doc = #doc_slot]
            #[derive(Clone, Copy, Debug, PartialEq, Eq)]
            pub struct #slot_ty {
                pub field: #field_ty,
                pub count: #count_ty,
                pub kind: Option<#kind_ty>,
            }

            impl #kind_ty {
                /// The children of this kind in source order; empty for kinds without children.
                #[must_use]
                #[allow(clippy::too_many_lines, clippy::match_same_arms, reason = "one arm per kind, as declared")]
                pub const fn slots(self) -> &'static [#slot_ty] {
                    match self {
                        #(#arms)*
                        _ => &[],
                    }
                }
            }
        })
    }
}
