//! The implementations derived: fields in declaration order, after a variant's position.

use proc_macro::{Ident, Span, TokenStream};

use crate::item::{Body, Fields, Item, ParamKind};
use crate::tokens::Tokens;

/// One shape a value can have: the path constructing it, the tag naming it among an enum's
/// variants, and its fields.
struct Case<'i> {
    path: Tokens,
    tag: Option<u32>,
    fields: &'i Fields,
}

impl Item {
    pub(crate) fn encode(&self) -> TokenStream {
        let cases = self.cases();
        let arms = cases.iter().fold(Tokens::new(), |arms, case| {
            let tag = case.tag.map(|tag| Tokens::new().encoding(Tokens::new().op("&").number(tag)));
            let fields =
                case.bindings().into_iter().map(|field| Tokens::new().encoding(Tokens::new().tree(field)));
            let body = tag.into_iter().chain(fields).fold(Tokens::new(), Tokens::then);
            arms.then(case.pattern()).op("=>").braces(body)
        });
        // An enum without variants has no value to match a reference to.
        let scrutinee = if cases.is_empty() { Tokens::new().op("*") } else { Tokens::new() };
        let method = Tokens::new()
            .word("fn")
            .word("encode")
            .parens(Tokens::new().op("&").word("self").op(",").word("w").op(":").op("&").word("mut").writer())
            .braces(Tokens::new().word("match").then(scrutinee).word("self").braces(arms));
        self.implement("Encode", method)
    }

    pub(crate) fn decode(&self) -> TokenStream {
        let body = match &self.body {
            Body::Struct(_) => Tokens::new().ok(self.cases().remove(0).construct()),
            Body::Enum(_) => {
                let arms = self.cases().into_iter().fold(Tokens::new(), |arms, case| {
                    let tag = case.tag.expect("a variant has a tag");
                    arms.number(tag).op("=>").ok(case.construct()).op(",")
                });
                let tag = Tokens::new().word("tag");
                let malformed = Tokens::new().codec("Malformed").op("::").word("Tag");
                let widened = Tokens::new().path(&["core", "convert", "From", "from"]).parens(tag.clone());
                let unknown = Tokens::new()
                    .path(&["core", "result", "Result", "Err"])
                    .parens(malformed.parens(widened));
                // The tags are `u32` literals, which type the value read.
                let read = Tokens::new().decoding();
                Tokens::new().word("match").then(read).braces(arms.then(tag).op("=>").then(unknown))
            }
        };
        let result = Tokens::new()
            .path(&["core", "result", "Result"])
            .op("<")
            .word("Self")
            .op(",")
            .codec("Malformed")
            .op(">");
        let method = Tokens::new()
            .word("fn")
            .word("decode")
            .parens(Tokens::new().word("r").op(":").op("&").word("mut").reader())
            .op("->")
            .then(result)
            .braces(body);
        self.implement("Decode", method)
    }

    /// `impl<P> ::vun_codec::Trait for Name<P> where P: ::vun_codec::Trait { method }`.
    fn implement(&self, name: &str, method: Tokens) -> TokenStream {
        let params = &self.generics.params;
        let declarations = params.iter().map(|p| Tokens::new().all(p.declaration.iter().cloned()));
        let arguments = params.iter().map(|p| Tokens::new().all(p.name.iter().cloned()));
        let bounded = params.iter().filter(|p| p.kind == ParamKind::Type);
        let bounds = bounded.map(|p| Tokens::new().all(p.name.iter().cloned()).op(":").codec(name).op(","));
        let clause = Tokens::new().all(self.generics.predicates.iter().cloned());
        let clause = bounds.fold(clause, Tokens::then);
        let lint = Tokens::new().word("allow").parens(Tokens::new().word("unused_variables"));
        Tokens::new()
            .attr(Tokens::new().word("automatically_derived"))
            .word("impl")
            .angled(declarations)
            .codec(name)
            .word("for")
            .tree(self.name.clone())
            .angled(arguments)
            .then(if clause.is_empty() { clause } else { Tokens::new().word("where").then(clause) })
            .braces(Tokens::new().attr(lint).then(method))
            .into()
    }

    fn cases(&self) -> Vec<Case<'_>> {
        let own = || Tokens::new().word("Self");
        match &self.body {
            Body::Struct(fields) => vec![Case { path: own(), tag: None, fields }],
            Body::Enum(variants) => (0..)
                .zip(variants)
                .map(|(tag, (name, fields))| Case {
                    path: own().op("::").tree(name.clone()),
                    tag: Some(tag),
                    fields,
                })
                .collect(),
        }
    }
}

impl Case<'_> {
    /// The bindings of its fields, in order: named fields by their names.
    fn bindings(&self) -> Vec<Ident> {
        match self.fields {
            Fields::Named(names) => names.clone(),
            Fields::Tuple(n) => (0..*n).map(|i| Ident::new(&format!("f{i}"), Span::call_site())).collect(),
            Fields::Unit => Vec::new(),
        }
    }

    /// The pattern binding its fields, as `Self::Variant { a, b }`.
    fn pattern(&self) -> Tokens {
        let fields = Tokens::new().comma(self.bindings(), Tokens::tree);
        match self.fields {
            Fields::Named(_) => self.path.clone().braces(fields),
            Fields::Tuple(_) => self.path.clone().parens(fields),
            Fields::Unit => self.path.clone(),
        }
    }

    /// Its value, each field decoded in order.
    fn construct(self) -> Tokens {
        match self.fields {
            Fields::Named(names) => self
                .path
                .braces(Tokens::new().comma(names, |t, name| t.tree(name.clone()).op(":").decoding())),
            Fields::Tuple(n) => self.path.parens(Tokens::new().comma(0..*n, |t, _| t.decoding())),
            Fields::Unit => self.path,
        }
    }
}
