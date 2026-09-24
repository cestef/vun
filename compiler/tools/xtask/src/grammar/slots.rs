//! The children of kinds whose entries are written `.name(.count(.field), ...)`.
//!
//! A kind `NodeKind` with such entries generates three modules: `NodeField`, the names of all its
//! fields, `NodeSlot { field, count, kind }`, and `NodeKind::slots`, each kind's slots in source
//! order. The counts are the variants of one declared enum. A field named after a variant of the
//! kind holds a node of that kind; any other field holds any node.

use quote::quote;

use super::{Enum, Grammar};
use crate::generate::{Access, Module, Read};
use crate::names::{constant, ident, pascal, snake};

/// Suffix removed from a kind's name to name its field and slot types.
const KIND_SUFFIX: &str = "Kind";

impl Grammar<'_> {
    pub(super) fn slots(&self, kind: &Enum<'_>) -> Read<Vec<Module>> {
        let with_slots: Vec<_> = kind.variants.iter().filter(|v| !v.slots.is_empty()).collect();
        if with_slots.is_empty() {
            return Ok(Vec::new());
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
            let (variant, name) = (pascal(v.name), constant(v.name));
            quote! { Self::#variant => #name, }
        });
        let tables = with_slots.iter().map(|v| {
            let variant = constant(v.name);
            let slots = v.slots.iter().map(|s| {
                let (field, count) = (pascal(s.field), pascal(s.count));
                let restricted = kind.variants.iter().any(|k| k.name == s.field);
                let node = if restricted {
                    quote! { Some(#kind_ty::#field) }
                } else {
                    quote! { None }
                };
                quote! { #slot_ty::new(#field_ty::#field, #count_ty::#count, #node) }
            });
            quote! { const #variant: &[#slot_ty] = &[#(#slots),*]; }
        });

        let doc_field = format!(" The named children of [`{}`] kinds.", kind.name);
        let doc_slot = format!(
            " One child position of a [`{}`]: its field, how many nodes it holds, and the kind they must have, if restricted.",
            kind.name
        );
        let field = quote! {
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
        };
        let slot = quote! {
            #[doc = #doc_slot]
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
        };
        let slots = quote! {
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

            #(#tables)*
        };
        Ok(vec![
            Module::sibling(snake(&field_ty.to_string()), &field),
            Module::sibling(snake(&slot_ty.to_string()), &slot),
            Module { access: Access::Impl, ..Module::sibling(format!("{}_slots", snake(kind.name)), &slots) },
        ])
    }
}
