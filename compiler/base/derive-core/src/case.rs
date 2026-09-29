//! The shapes a value of the item takes, one per variant or one for a struct, and the `impl` a
//! derive writes around what it generates from them.

use proc_macro::TokenStream;

use crate::item::{Body, Fields, Item, ParamKind, Variant};
use crate::tokens::Tokens;

/// One shape a value can have: the path constructing it, its variant and the variant's position
/// in an enum, and its fields.
pub struct Case<'i> {
    pub path: Tokens,
    pub index: Option<u32>,
    pub variant: Option<&'i Variant>,
    pub fields: &'i Fields,
}

impl Item {
    /// Its cases: the struct's one, or one per variant in order.
    #[must_use]
    pub fn cases(&self) -> Vec<Case<'_>> {
        let own = || Tokens::new().word("Self");
        match &self.body {
            Body::Struct(fields) => vec![Case { path: own(), index: None, variant: None, fields }],
            Body::Enum(variants) => (0..)
                .zip(variants)
                .map(|(index, v)| Case {
                    path: own().op("::").tree(v.name.clone()),
                    index: Some(index),
                    variant: Some(v),
                    fields: &v.fields,
                })
                .collect(),
        }
    }

    /// `impl<P> Trait for Name<P> where P: Trait { items }`, every type parameter bound by
    /// `trait_path`.
    #[must_use]
    pub fn implement(&self, trait_path: &Tokens, items: Tokens) -> TokenStream {
        let params = &self.generics.params;
        let declarations = params.iter().map(|p| Tokens::new().all(p.declaration.iter().cloned()));
        let arguments = params.iter().map(|p| Tokens::new().all(p.name.iter().cloned()));
        let bounded = params.iter().filter(|p| p.kind == ParamKind::Type);
        let bounds = bounded
            .map(|p| Tokens::new().all(p.name.iter().cloned()).op(":").then(trait_path.clone()).op(","));
        let clause = Tokens::new().all(self.generics.predicates.iter().cloned());
        let clause = bounds.fold(clause, Tokens::then);
        let lint = Tokens::new().word("allow").parens(Tokens::new().word("unused_variables"));
        Tokens::new()
            .attr(Tokens::new().word("automatically_derived"))
            .word("impl")
            .angled(declarations)
            .then(trait_path.clone())
            .word("for")
            .tree(self.name.clone())
            .angled(arguments)
            .then(if clause.is_empty() { clause } else { Tokens::new().word("where").then(clause) })
            .braces(Tokens::new().attr(lint).then(items))
            .into()
    }
}

impl Case<'_> {
    /// The pattern binding its fields, as `Self::Variant { a, b }` or `Self::Variant(f0, f1)`.
    pub fn pattern(&self) -> Tokens {
        let fields = Tokens::new().comma(self.fields.all(), |t, f| t.tree(f.binding.clone()));
        match self.fields {
            Fields::Named(_) => self.path.clone().braces(fields),
            Fields::Tuple(_) => self.path.clone().parens(fields),
            Fields::Unit => self.path.clone(),
        }
    }

    /// Its value, its fields taking `values` in order.
    pub fn construct(&self, values: impl IntoIterator<Item = Tokens>) -> Tokens {
        let fields = self.fields.all().iter().zip(values);
        match self.fields {
            Fields::Named(_) => self
                .path
                .clone()
                .braces(Tokens::new().comma(fields, |t, (f, v)| t.tree(f.binding.clone()).op(":").then(v))),
            Fields::Tuple(_) => self.path.clone().parens(Tokens::new().comma(fields, |t, (_, v)| t.then(v))),
            Fields::Unit => self.path.clone(),
        }
    }
}
