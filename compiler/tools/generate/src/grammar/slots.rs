//! The children of kinds whose entries are productions, `.name(terms...)`.
//!
//! A kind `NodeKind` with productions generates `NodeField`, the names of all its fields,
//! `NodeSlot { field, count, kind }`, one child position, `NodeTerm`, a term of a production, a
//! module per list of the kind holding each entry's slots and terms, and `NodeKind::slots` and
//! `NodeKind::production` reading them. The counts are the variants of one declared enum; the
//! tokens terms name are variants of the one kind that has them all. A field named after a
//! variant of the kind holds a node of that kind; any other field holds any node.

use std::borrow::Cow;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use vun_syntax::literal::Decl;

use super::production::{Resolved, Restriction, Term, Tokens, Written};
use super::{Enum, Grammar, Slot, Variant};
use crate::generate::{Access, Module, Read};
use crate::names::{Name, constant, ident, pascal};

/// Suffix removed from a kind's name to name its field, slot and term types.
const KIND_SUFFIX: &str = "Kind";

/// Ends the name of a kind's field type.
const FIELD: &str = "Field";

/// Ends the name of an entry's slots, beside its terms: `BINDING_SLOTS`.
const SLOTS: &str = "_SLOTS";

/// The Rust names of a kind's generated types.
struct Types {
    kind: proc_macro2::Ident,
    field: proc_macro2::Ident,
    slot: proc_macro2::Ident,
    term: proc_macro2::Ident,
    count: proc_macro2::Ident,
}

impl<'s> Enum<'s> {
    /// The name of the kind with types named after it: `NodeKind` names `Node...`.
    fn base(&self) -> &str {
        self.name.strip_suffix(KIND_SUFFIX).unwrap_or(&self.name)
    }

    /// For a kind with productions, the enum of its fields, in the order they first stand, which
    /// tables may name.
    pub(super) fn fields(&self) -> Read<Option<Self>> {
        let mut fields: Vec<&'s str> = Vec::new();
        for v in &self.variants {
            let Some(terms) = &v.production else { continue };
            for slot in Term::slots(v.name, terms)? {
                if !fields.contains(&slot.field) {
                    fields.push(slot.field);
                }
            }
        }
        Ok((!fields.is_empty()).then(|| Self {
            name: Cow::Owned(format!("{}{FIELD}", self.base())),
            doc: Vec::new(),
            variants: fields
                .into_iter()
                .map(|name| Variant { name, doc: Vec::new(), text: None, production: None })
                .collect(),
            lists: Vec::new(),
        }))
    }
}

impl<'s> Grammar<'s> {
    pub(super) fn slots(&self, kind: &Enum<'s>, decls: &[Decl<'s>]) -> Read<Vec<Module>> {
        let produced: Vec<(&Variant<'s>, &[Term<'s, Written<'s>>])> =
            kind.variants.iter().filter_map(|v| Some((v, v.production.as_deref()?))).collect();
        if produced.is_empty() {
            return Ok(Vec::new());
        }
        let slots: Vec<Vec<Slot<'s>>> =
            produced.iter().map(|(v, terms)| Term::slots(v.name, terms)).collect::<Read<_>>()?;
        let counts: Vec<&str> = slots.iter().flat_map(|s| s.iter().map(|s| s.count)).collect();
        let base = kind.base();
        let types = Types {
            kind: ident(&kind.name),
            field: format_ident!("{base}{FIELD}"),
            slot: format_ident!("{base}Slot"),
            term: format_ident!("{base}Term"),
            count: ident(&self.enums[self.resolve(&kind.name, &counts)?].name),
        };
        let tokens = Tokens::of(self, decls, kind)?;
        let field_enum =
            self.enums.iter().find(|e| *e.name == *types.field.to_string()).expect("read derives it");
        let fields: Vec<&str> = field_enum.variants.iter().map(|v| v.name).collect();
        let mut modules = vec![
            Module::sibling(types.field.to_string().snake(), &types.field(kind, &fields)),
            Module::sibling(types.slot.to_string().snake(), &types.slot(kind)),
            Module::sibling(types.term.to_string().snake(), &types.term(kind, &ident(&tokens.kind.name))),
        ];
        let mut slot_arms = Vec::new();
        let mut term_arms = Vec::new();
        for (list, members) in &kind.lists {
            let module = format_ident!("{}_{list}", kind.name.snake());
            let mut items = Vec::new();
            for (i, (v, terms)) in produced.iter().enumerate().filter(|(_, (v, _))| members.contains(&v.name))
            {
                let (variant, name) = (pascal(v.name), constant(v.name));
                let slots_name = format_ident!("{name}{SLOTS}");
                let terms = types.slice(tokens.kind, &tokens.resolve(v.name, terms)?);
                let (slot_ty, term_ty) = (&types.slot, &types.term);
                items.push(quote! { pub(super) const #name: &[#term_ty] = #terms; });
                if !slots[i].is_empty() {
                    let own = slots[i].iter().map(|s| types.slot_value(kind, s));
                    items.push(quote! { pub(super) const #slots_name: &[#slot_ty] = &[#(#own),*]; });
                    slot_arms.push(quote! { Self::#variant => #module::#slots_name, });
                }
                term_arms.push(quote! { Self::#variant => Some(#module::#name), });
            }
            if !items.is_empty() {
                let items = quote! { #(#items)* };
                modules
                    .push(Module { access: Access::Sibling, ..Module::sibling(module.to_string(), &items) });
            }
        }
        let unproduced: Vec<_> =
            kind.variants.iter().filter(|v| v.production.is_none()).map(|v| pascal(v.name)).collect();
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

impl Types {
    fn field(&self, kind: &Enum<'_>, fields: &[&str]) -> TokenStream {
        let field_ty = &self.field;
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

    fn term(&self, kind: &Enum<'_>, tokens: &proc_macro2::Ident) -> TokenStream {
        let (term_ty, field_ty, kind_ty) = (&self.term, &self.field, &self.kind);
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

    /// `terms` as a Rust slice of terms, their tokens variants of `tokens`.
    fn slice(&self, tokens: &Enum<'_>, terms: &[Term<'_, Resolved<'_>>]) -> TokenStream {
        let terms = terms.iter().map(|t| self.value(tokens, t));
        quote! { &[#(#terms),*] }
    }

    fn value(&self, tokens: &Enum<'_>, term: &Term<'_, Resolved<'_>>) -> TokenStream {
        let ty = &self.term;
        match term {
            Term::Leaf(Resolved::Tokens(names)) => {
                let (kind, names) = (ident(&tokens.name), names.iter().map(|n| pascal(n)));
                quote! { #ty::Token(&[#(#kind::#names),*]) }
            }
            Term::Leaf(Resolved::Words(words)) => {
                let words = words.iter().map(AsRef::<str>::as_ref);
                quote! { #ty::Word(&[#(#words),*]) }
            }
            Term::Field(field, Some(restriction)) => {
                let (field_ty, variant) = (&self.field, pascal(field));
                let (form, table) = match restriction {
                    Restriction::Of(table) => (ident("Of"), constant(table)),
                    Restriction::Ending(table) => (ident("Ending"), constant(table)),
                    Restriction::Starting(table) => (ident("Starting"), constant(table)),
                };
                quote! { #ty::#form(#field_ty::#variant, #table) }
            }
            Term::Field(field, None) => {
                let (field_ty, variant) = (&self.field, pascal(field));
                quote! { #ty::Field(#field_ty::#variant) }
            }
            Term::Optional(terms) => self.compound(tokens, "Optional", terms),
            Term::Many(terms) => self.compound(tokens, "Many", terms),
            Term::Choice(terms) => self.compound(tokens, "Choice", terms),
            Term::Sequence(terms) => self.compound(tokens, "Sequence", terms),
            Term::Separated(item, separator) => {
                let (item, separator) = (self.value(tokens, item), self.value(tokens, separator));
                quote! { #ty::Separated(&#item, &#separator) }
            }
        }
    }

    fn compound(&self, tokens: &Enum<'_>, variant: &str, terms: &[Term<'_, Resolved<'_>>]) -> TokenStream {
        let (ty, variant, terms) = (&self.term, ident(variant), self.slice(tokens, terms));
        quote! { #ty::#variant(#terms) }
    }

    fn slot_value(&self, kind: &Enum<'_>, s: &Slot<'_>) -> TokenStream {
        let (slot_ty, field_ty, count_ty, kind_ty) = (&self.slot, &self.field, &self.count, &self.kind);
        let (field, count) = (pascal(s.field), pascal(s.count));
        let node = if kind.variants.iter().any(|k| k.name == s.field) {
            quote! { Some(#kind_ty::#field) }
        } else {
            quote! { None }
        };
        quote! { #slot_ty::new(#field_ty::#field, #count_ty::#count, #node) }
    }
}
