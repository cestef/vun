//! The children of kinds whose entries are productions, `.name(terms...)`.
//!
//! A kind `NodeKind` with productions generates `NodeField`, the names of all its fields,
//! `NodeSlot { field, count, kind }`, one child position, `NodeTerm`, a term of a production, a
//! module per list of the kind holding each entry's slots and terms, and `NodeKind::slots` and
//! `NodeKind::production` reading them. The counts are the variants of one declared enum; the
//! tokens terms name are variants of the one kind that has them all. A field named after a
//! variant of the kind holds a node of that kind; any other field holds any node.

use quote::{format_ident, quote};

use std::borrow::Cow;

use super::model::{Count, Model, Term};
use super::{Enum, Grammar, Variant};

mod types;
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
        let (mut slot_arms, mut term_arms, mut start_arms) = (Vec::new(), Vec::new(), Vec::new());
        for (list, members) in &model.nodes.lists {
            let module = format_ident!("{}_{list}", kind.name.snake());
            let mut items = Vec::new();
            for &member in members {
                let Some(terms) = model.production(member) else { continue };
                let name = model.kind_name(member);
                let (variant, constant) = (pascal(name), constant(name));
                let slots_name = format_ident!("{constant}{SLOTS}");
                let (slot_ty, term_ty, spelled) = (&types.slot, &types.term, types.slice(terms));
                items.push(quote! { pub(super) const #constant: &[#term_ty] = #spelled; });
                let slots = model.slots(member);
                if !slots.is_empty() {
                    let own = slots.iter().map(|s| types.slot_value(*s));
                    items.push(quote! { pub(super) const #slots_name: &[#slot_ty] = &[#(#own),*]; });
                    slot_arms.push(quote! { Self::#variant => #module::#slots_name, });
                }
                term_arms.push(quote! { Self::#variant => Some(#module::#constant), });
                if let Some(Term::Tokens(first)) = terms.first() {
                    let (tokens_ty, first) =
                        (&types.tokens, first.iter().map(|&t| pascal(model.token_name(t))));
                    start_arms.push(quote! { Self::#variant => &[#(#tokens_ty::#first),*], });
                }
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
        let (kind_ty, slot_ty, term_ty, tokens_ty) = (&types.kind, &types.slot, &types.term, &types.tokens);
        let infix_form = model.infix_kind.map(|kind| {
            let variant = pascal(model.kind_name(kind));
            quote! {
                /// The form spelling the operators of the table `infix`, whose levels stand for its
                /// row of `precedence`.
                pub const INFIX_FORM: Self = Self::#variant;
            }
        });
        let lookups = quote! {
            impl #kind_ty {
                #infix_form

                /// The children of this kind in source order; empty for kinds without children.
                #[must_use]
                #[allow(clippy::too_many_lines, reason = "one arm per kind, as declared")]
                pub const fn slots(self) -> &'static [#slot_ty] {
                    match self {
                        #(#slot_arms)*
                        _ => &[],
                    }
                }

                /// The tokens this kind's production may start with; empty where it starts with a
                /// child.
                #[must_use]
                #[allow(
                    clippy::too_many_lines,
                    clippy::match_same_arms,
                    reason = "one arm per kind, as declared"
                )]
                pub const fn starts(self) -> &'static [#tokens_ty] {
                    match self {
                        #(#start_arms)*
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
