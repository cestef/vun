//! Productions: what a kind's entry `.name(terms...)` spells, read by shape (the forms are in the
//! doc of `NodeKind` in `grammar.vun`), the fields and counts derived from it, and its terms
//! written as Rust values once the tokens they name are resolved.

use std::borrow::Cow;

use proc_macro2::TokenStream;
use quote::quote;
use vun_diag::Msg;
use vun_syntax::literal::{Decl, Literal};

use super::{Enum, GENERATOR, Grammar, SHAPE, Slot, UNRESOLVED};
use crate::generate::Read;
use crate::names::{Name, ident, pascal};

/// The tags of the compound terms.
const TOKEN: &str = "token";
const WORD: &str = "word";
const OPTIONAL: &str = "optional";
const MANY: &str = "many";
const SEPARATED: &str = "separated";
const CHOICE: &str = "choice";

/// The count of a field outside any compound term; the others are the tags above.
const ONE: &str = "one";

/// A term of a production, as written.
pub(super) enum Term<'s> {
    /// A token or word by its spelling.
    Text(Cow<'s, str>),
    /// A token of these kinds, lists of the token kind, or tables of tokens.
    Token(Vec<&'s str>),
    /// A name or keyword spelled as a variant of this kind.
    Word(&'s str),
    Field(&'s str),
    Optional(Vec<Term<'s>>),
    Many(Vec<Term<'s>>),
    Separated(Box<Term<'s>>, Box<Term<'s>>),
    Choice(Vec<Term<'s>>),
    Sequence(Vec<Term<'s>>),
}

impl<'s> Term<'s> {
    pub(super) fn read(value: &Literal<'s>) -> Option<Self> {
        let all = |values: &[Literal<'s>]| values.iter().map(Self::read).collect::<Option<Vec<_>>>();
        let names = |values: &[Literal<'s>]| values.iter().map(Literal::as_name).collect::<Option<Vec<_>>>();
        Some(match value {
            Literal::String(text) => Self::Text(text.clone()),
            Literal::Name(field) => Self::Field(field),
            Literal::List(terms) => Self::Sequence(all(terms)?),
            Literal::Tagged(tag, payload) => match (*tag, payload.as_slice()) {
                (TOKEN, kinds) if !kinds.is_empty() => Self::Token(names(kinds)?),
                (WORD, [Literal::Name(kind)]) => Self::Word(kind),
                (OPTIONAL, terms) => Self::Optional(all(terms)?),
                (MANY, terms) => Self::Many(all(terms)?),
                (SEPARATED, [item, separator]) => {
                    Self::Separated(Box::new(Self::read(item)?), Box::new(Self::read(separator)?))
                }
                (CHOICE, terms) => Self::Choice(all(terms)?),
                _ => return None,
            },
            _ => return None,
        })
    }

    /// The fields of `terms`, in the order they stand, each with its count: `many` in `many` or
    /// `separated`, else `optional` in `optional` or `choice`, else `one`. A field stands once.
    pub(super) fn slots(owner: &str, terms: &[Self]) -> Read<Vec<Slot<'s>>> {
        let mut slots: Vec<Slot<'s>> = Vec::new();
        for t in terms {
            t.fields(ONE, &mut slots);
        }
        for (i, slot) in slots.iter().enumerate() {
            if slots[..i].iter().any(|s| s.field == slot.field) {
                let msg = Msg::new().text("the field ").code(slot.field).text(" stands twice");
                return Err(GENERATOR.error(SHAPE, owner, msg));
            }
        }
        Ok(slots)
    }

    fn fields(&self, count: &'s str, into: &mut Vec<Slot<'s>>) {
        let looser = |tag: &'s str| if count == MANY { MANY } else { tag };
        let (terms, count): (Vec<&Self>, _) = match self {
            Self::Field(field) => return into.push(Slot { count, field }),
            Self::Text(_) | Self::Token(_) | Self::Word(_) => return,
            Self::Sequence(terms) => (terms.iter().collect(), count),
            Self::Optional(terms) | Self::Choice(terms) => (terms.iter().collect(), looser(OPTIONAL)),
            Self::Many(terms) => (terms.iter().collect(), MANY),
            Self::Separated(item, separator) => (vec![item, separator], MANY),
        };
        for t in terms {
            t.fields(count, into);
        }
    }

    /// The token names this term writes directly, tables and lists aside.
    fn tokens(&self, grammar: &Grammar<'s>, decls: &[Decl<'s>], into: &mut Vec<&'s str>) {
        match self {
            Self::Token(names) => {
                into.extend(names.iter().filter(|n| Tokens::expanded(grammar, decls, n).is_none()));
            }
            Self::Text(_) | Self::Word(_) | Self::Field(_) => {}
            Self::Sequence(terms) | Self::Optional(terms) | Self::Many(terms) | Self::Choice(terms) => {
                for t in terms {
                    t.tokens(grammar, decls, into);
                }
            }
            Self::Separated(item, separator) => {
                item.tokens(grammar, decls, into);
                separator.tokens(grammar, decls, into);
            }
        }
    }
}

/// What terms are written with: the grammar, its declarations and the kind of tokens, the one
/// kind with every token name the productions write.
pub(super) struct Tokens<'g, 's> {
    grammar: &'g Grammar<'s>,
    decls: &'g [Decl<'s>],
    kind: &'g Enum<'s>,
    /// The Rust types of terms, fields and tokens.
    pub(super) term: proc_macro2::Ident,
    pub(super) field: proc_macro2::Ident,
}

impl<'g, 's> Tokens<'g, 's> {
    pub(super) fn of(
        grammar: &'g Grammar<'s>,
        decls: &'g [Decl<'s>],
        of: &Enum<'s>,
        (term, field): (proc_macro2::Ident, proc_macro2::Ident),
    ) -> Read<Self> {
        let mut names = Vec::new();
        for v in &of.variants {
            v.production.iter().flatten().for_each(|t| t.tokens(grammar, decls, &mut names));
        }
        let kind = &grammar.enums[grammar.resolve(of.name, &names)?];
        Ok(Self { grammar, decls, kind, term, field })
    }

    /// The token names a list of the kind or a table stands for, or `None` for another name.
    fn expanded(grammar: &Grammar<'s>, decls: &[Decl<'s>], name: &str) -> Option<Vec<&'s str>> {
        if let Some(table) = decls.iter().find(|d| d.name == name && !d.name.is_type_name()) {
            let mut names = Vec::new();
            Self::mentioned(&table.value, &mut names);
            return Some(names);
        }
        let mut lists = grammar.enums.iter().flat_map(|e| &e.lists);
        lists.find(|(list, _)| *list == name).map(|(_, names)| names.clone())
    }

    /// Every `.name` a value holds, at any depth.
    fn mentioned(value: &Literal<'s>, into: &mut Vec<&'s str>) {
        match value {
            Literal::Name(name) => into.push(name),
            Literal::List(items) => items.iter().for_each(|v| Self::mentioned(v, into)),
            Literal::Record(fields) => fields.iter().for_each(|f| Self::mentioned(&f.value, into)),
            _ => {}
        }
    }

    /// `terms` as a Rust slice of terms.
    pub(super) fn slice(&self, owner: &str, terms: &[Term<'s>]) -> Read<TokenStream> {
        let terms = terms.iter().map(|t| self.term(owner, t)).collect::<Read<Vec<_>>>()?;
        Ok(quote! { &[#(#terms),*] })
    }

    fn term(&self, owner: &str, term: &Term<'s>) -> Read<TokenStream> {
        let ty = &self.term;
        Ok(match term {
            Term::Text(text) => match self.kind.variants.iter().find(|v| v.text.as_deref() == Some(text)) {
                Some(v) => self.token_slice(&[v.name]),
                None if text.chars().all(|c| c.is_alphanumeric() || c == '_') => {
                    quote! { #ty::Word(&[#text]) }
                }
                None => {
                    let msg = Msg::new().text("no token is spelled ").code(text.as_ref());
                    return Err(GENERATOR.error(UNRESOLVED, owner, msg));
                }
            },
            Term::Token(names) => {
                let mut kinds = Vec::new();
                for name in names {
                    match Self::expanded(self.grammar, self.decls, name) {
                        Some(listed) => kinds.extend(listed.into_iter().filter(|n| self.has(n))),
                        None => kinds.push(name),
                    }
                }
                self.token_slice(&kinds)
            }
            Term::Word(kind) => {
                let words = self.grammar.enums.iter().find(|e| e.name == *kind).ok_or_else(|| {
                    GENERATOR.error(UNRESOLVED, owner, Msg::new().text("no kind is named ").code(*kind))
                })?;
                let texts = words.variants.iter().map(|v| v.text.as_deref()).collect::<Option<Vec<_>>>();
                let texts = texts.ok_or_else(|| {
                    GENERATOR.error(SHAPE, owner, Msg::new().code(*kind).text(" has an unspelled variant"))
                })?;
                quote! { #ty::Word(&[#(#texts),*]) }
            }
            Term::Field(field) => {
                let (field_ty, variant) = (&self.field, pascal(field));
                quote! { #ty::Field(#field_ty::#variant) }
            }
            Term::Optional(terms) => self.compound(owner, "Optional", terms)?,
            Term::Many(terms) => self.compound(owner, "Many", terms)?,
            Term::Choice(terms) => self.compound(owner, "Choice", terms)?,
            Term::Sequence(terms) => self.compound(owner, "Sequence", terms)?,
            Term::Separated(item, separator) => {
                let (item, separator) = (self.term(owner, item)?, self.term(owner, separator)?);
                quote! { #ty::Separated(&#item, &#separator) }
            }
        })
    }

    fn compound(&self, owner: &str, variant: &str, terms: &[Term<'s>]) -> Read<TokenStream> {
        let (ty, variant, terms) = (&self.term, ident(variant), self.slice(owner, terms)?);
        Ok(quote! { #ty::#variant(#terms) })
    }

    /// The Rust type of the tokens.
    pub(super) fn kind_ident(&self) -> proc_macro2::Ident {
        ident(self.kind.name)
    }

    fn has(&self, name: &str) -> bool {
        self.kind.variants.iter().any(|v| v.name == name)
    }

    fn token_slice(&self, names: &[&str]) -> TokenStream {
        let (ty, kind) = (&self.term, ident(self.kind.name));
        let kinds = names.iter().map(|n| pascal(n));
        quote! { #ty::Token(&[#(#kind::#kinds),*]) }
    }
}
