//! The children of kinds whose entries are productions, `.name(terms...)`.
//!
//! A kind `NodeKind` with productions generates `NodeField`, the names of all its fields,
//! `NodeSlot { field, count, kind }`, one child position, `NodeTerm`, a term of a production, a
//! module per list of the kind holding each entry's slots and terms, and `NodeKind::slots` and
//! `NodeKind::production` reading them. The counts are the variants of one declared enum; the
//! tokens terms name are variants of the one kind that has them all. A field named after a
//! variant of the kind holds a node of that kind; any other field holds any node.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use std::borrow::Cow;

use super::model::{Count, Model, Restriction, Slot, Term};
use super::{Enum, Grammar, Variant};
use crate::generate::{Access, Module, Read};
use crate::names::{Name, constant, ident, pascal};

/// Suffix removed from a kind's name to name its field, slot and term types.
const KIND_SUFFIX: &str = "Kind";

/// Ends the name of a kind's field type.
const FIELD: &str = "Field";

/// Ends the name of an entry's slots, beside its terms: `BINDING_SLOTS`.
const SLOTS: &str = "_SLOTS";

/// The Rust names of a kind's generated types.
struct Types<'m> {
    model: &'m Model,
    kind: proc_macro2::Ident,
    field: proc_macro2::Ident,
    slot: proc_macro2::Ident,
    term: proc_macro2::Ident,
    count: proc_macro2::Ident,
    tokens: proc_macro2::Ident,
}

impl Enum<'_> {
    /// The name of the kind with types named after it: `NodeKind` names `Node...`.
    fn base(&self) -> &str {
        self.name.strip_suffix(KIND_SUFFIX).unwrap_or(&self.name)
    }

    /// For the kind of the model's nodes, the enum of its fields, which tables may name.
    pub(super) fn fields(&self, model: &Model) -> Enum<'static> {
        let variants =
            model.fields.iter().map(|f| Variant { name: Cow::Owned(f.clone()), doc: Vec::new(), text: None });
        Enum {
            name: Cow::Owned(format!("{}{FIELD}", self.base())),
            doc: Vec::new(),
            variants: variants.collect(),
            lists: Vec::new(),
        }
    }

    /// Whether this is the kind of the model's nodes: it has their variants.
    pub(super) fn is_nodes(&self, model: &Model) -> bool {
        self.variants.iter().map(|v| v.name.as_ref()).eq(model.nodes.variants.iter().map(|v| v.name.as_str()))
    }
}

impl Grammar<'_> {
    /// For the kind of the model's nodes, its fields, slots, terms and productions.
    pub(super) fn slots(&self, kind: &Enum<'_>, model: &Model) -> Read<Vec<Module>> {
        if !kind.is_nodes(model) {
            return Ok(Vec::new());
        }
        let counts: Vec<&str> = Count::ALL.iter().map(|c| c.name()).collect();
        let tokens: Vec<&str> = model.all_tokens().map(|t| model.token_name(t)).collect();
        let base = kind.base();
        let types = Types {
            model,
            kind: ident(&kind.name),
            field: format_ident!("{base}{FIELD}"),
            slot: format_ident!("{base}Slot"),
            term: format_ident!("{base}Term"),
            count: ident(&self.enums[self.resolve(&kind.name, &counts)?].name),
            tokens: ident(&self.enums[self.resolve(&kind.name, &tokens)?].name),
        };
        let mut modules = vec![
            Module::sibling(types.field.to_string().snake(), &types.field(kind)),
            Module::sibling(types.slot.to_string().snake(), &types.slot(kind)),
            Module::sibling(types.term.to_string().snake(), &types.term(kind)),
        ];
        let (mut slot_arms, mut term_arms) = (Vec::new(), Vec::new());
        for (list, members) in &model.nodes.lists {
            let module = format_ident!("{}_{list}", kind.name.snake());
            let mut items = Vec::new();
            for &member in members {
                let Some(terms) = model.production(member) else { continue };
                let name = model.kind_name(member);
                let (variant, constant) = (pascal(name), constant(name));
                let slots_name = format_ident!("{constant}{SLOTS}");
                let (slot_ty, term_ty, terms) = (&types.slot, &types.term, types.slice(terms));
                items.push(quote! { pub(super) const #constant: &[#term_ty] = #terms; });
                let slots = model.slots(member);
                if !slots.is_empty() {
                    let own = slots.iter().map(|s| types.slot_value(*s));
                    items.push(quote! { pub(super) const #slots_name: &[#slot_ty] = &[#(#own),*]; });
                    slot_arms.push(quote! { Self::#variant => #module::#slots_name, });
                }
                term_arms.push(quote! { Self::#variant => Some(#module::#constant), });
            }
            if !items.is_empty() {
                let items = quote! { #(#items)* };
                modules
                    .push(Module { access: Access::Sibling, ..Module::sibling(module.to_string(), &items) });
            }
        }
        let unproduced: Vec<_> =
            model.nodes.variants.iter().filter(|v| v.production.is_none()).map(|v| pascal(&v.name)).collect();
        let unproduced = (!unproduced.is_empty()).then(|| quote! { #(Self::#unproduced)|* => None, });
        let (kind_ty, slot_ty, term_ty) = (&types.kind, &types.slot, &types.term);
        let lookups = quote! {
            impl #kind_ty {
                /// The children of this kind in source order; empty for kinds without children.
                #[must_use]
                #[allow(clippy::too_many_lines, reason = "one arm per kind, as declared")]
                pub const fn slots(self) -> &'static [#slot_ty] {
                    match self {
                        #(#slot_arms)*
                        _ => &[],
                    }
                }

                /// What this kind spells, in source order; `None` for a kind declared without a
                /// production.
                #[must_use]
                #[allow(clippy::too_many_lines, reason = "one arm per kind, as declared")]
                pub const fn production(self) -> Option<&'static [#term_ty]> {
                    match self {
                        #(#term_arms)*
                        #unproduced
                    }
                }
            }
        };
        modules.push(Module {
            access: Access::Impl,
            ..Module::sibling(format!("{}_productions", kind.name.snake()), &lookups)
        });
        Ok(modules)
    }
}

impl Types<'_> {
    fn field(&self, kind: &Enum<'_>) -> TokenStream {
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

    fn slot(&self, kind: &Enum<'_>) -> TokenStream {
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

    fn term(&self, kind: &Enum<'_>) -> TokenStream {
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
    fn slice(&self, terms: &[Term]) -> TokenStream {
        let terms = terms.iter().map(|t| self.value(t));
        quote! { &[#(#terms),*] }
    }

    fn value(&self, term: &Term) -> TokenStream {
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

    fn compound(&self, variant: &str, terms: &[Term]) -> TokenStream {
        let (ty, variant, terms) = (&self.term, ident(variant), self.slice(terms));
        quote! { #ty::#variant(#terms) }
    }

    fn slot_value(&self, s: Slot) -> TokenStream {
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
