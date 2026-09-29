//! The grammar resolved once for every emitter: the kinds of tokens and nodes, each production
//! with its tokens resolved and its fields numbered, and the tables the generator interprets, all
//! naming tokens, kinds and fields by typed ids. It is read by type from
//! `grammar.vun` and `nodes.vun` (`written`), and the editors' facts apart from `editors.vun`
//! ([`Editors`]); a name or path that resolves to nothing fails the read, naming it.

mod editors;
mod paths;
mod resolve;
mod term;
#[cfg(test)]
mod tests;
mod written;

use std::collections::BTreeMap;

pub(crate) use editors::{Captures, Editors};
pub(crate) use term::{Count, Restriction, Slot, Table, Term};
pub(crate) use written::Entry;

use crate::grammar::sublime::Recognized;

/// A kind of token, by its position in `TokenKind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Token(usize);

/// A kind of node, by its position in `NodeKind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Kind(usize);

/// A field of the node kinds, by the order fields first stand in productions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Field(usize);

/// A kind's variant: its name, its spelling if fixed, and for a node kind its production.
pub(crate) struct Variant<T> {
    pub(crate) name: String,
    pub(crate) text: Option<String>,
    pub(crate) production: Option<Vec<T>>,
}

/// A kind of `grammar.vun` or `nodes.vun`: its variants, and its lists of them.
pub(crate) struct Kinds<I, T> {
    pub(crate) variants: Vec<Variant<T>>,
    pub(crate) lists: Vec<(String, Vec<I>)>,
}

/// What a field holds besides the kind it is named after: the kinds a table lists, and whether
/// any operand too.
pub(crate) struct Contents {
    pub(crate) kinds: Vec<Kind>,
    pub(crate) operands: bool,
}

/// Fields whose operands, at any depth, read in a context: `kinds` are operands there, in place
/// of those starting with the same token.
pub(crate) struct OperandContext {
    pub(crate) fields: Vec<Field>,
    pub(crate) kinds: Vec<Kind>,
}

pub(crate) struct Model {
    pub(crate) tokens: Kinds<Token, ()>,
    pub(crate) nodes: Kinds<Kind, Term>,
    /// The fields' names, by [`Field`].
    pub(crate) fields: Vec<String>,
    /// The slots of each node kind's production, by [`Kind`].
    slots: Vec<Vec<Slot>>,
    /// Kinds that are never operands.
    pub(crate) parts: Vec<Kind>,
    pub(crate) contents: BTreeMap<Field, Contents>,
    pub(crate) operand_contexts: Vec<OperandContext>,
    /// Sets of kinds that may spell the same source, most preferred first.
    pub(crate) preferred: Vec<Vec<Kind>>,
    pub(crate) unterminated: Vec<Kind>,
    /// Kinds that end with their last child when written with one of the tokens.
    pub(crate) transparent: Vec<(Kind, Vec<Token>)>,
    pub(crate) line_ends: Vec<Kind>,
    /// Levels of the forms open at an edge, loosest first; the row holding the infix kind stands
    /// for the levels of [`Self::infix`].
    pub(crate) precedence: Vec<Vec<Kind>>,
    /// The operators of each infix level, loosest first.
    pub(crate) infix: Vec<Vec<Token>>,
    /// Opening and closing tokens.
    pub(crate) delimiters: Vec<(Token, Token)>,
    /// Words that are keywords only where their context says.
    pub(crate) contextual: Vec<(String, Recognized)>,
    /// The kind spread over the infix levels: its operator term takes their tokens.
    pub(crate) infix_kind: Option<Kind>,
}

impl Model {
    /// The names of the tables `.of` terms restrict children by: tables of node kinds.
    pub(crate) fn kind_tables(&self) -> Vec<&str> {
        fn walk<'m>(terms: &'m [Term], out: &mut Vec<&'m str>) {
            for term in terms {
                match term {
                    Term::Field(_, Some(Restriction::Of(table))) => out.push(&table.name),
                    Term::Optional(terms)
                    | Term::Many(terms)
                    | Term::Choice(terms)
                    | Term::Sequence(terms) => {
                        walk(terms, out);
                    }
                    Term::Separated(item, separator) => {
                        walk(std::slice::from_ref(item), out);
                        walk(std::slice::from_ref(separator), out);
                    }
                    Term::Tokens(_) | Term::Words(_) | Term::Field(..) => {}
                }
            }
        }
        let mut tables = Vec::new();
        for variant in &self.nodes.variants {
            walk(variant.production.as_deref().unwrap_or_default(), &mut tables);
        }
        tables
    }

    pub(crate) fn token(&self, name: &str) -> Option<Token> {
        self.tokens.variants.iter().position(|v| v.name == name).map(Token)
    }

    pub(crate) fn kind(&self, name: &str) -> Option<Kind> {
        self.nodes.variants.iter().position(|v| v.name == name).map(Kind)
    }

    pub(crate) fn field(&self, name: &str) -> Option<Field> {
        self.fields.iter().position(|f| f == name).map(Field)
    }

    pub(crate) fn token_name(&self, token: Token) -> &str {
        &self.tokens.variants[token.0].name
    }

    /// The fixed spelling of a token, if it has one.
    pub(crate) fn text(&self, token: Token) -> Option<&str> {
        self.tokens.variants[token.0].text.as_deref()
    }

    pub(crate) fn kind_name(&self, kind: Kind) -> &str {
        &self.nodes.variants[kind.0].name
    }

    pub(crate) fn field_name(&self, field: Field) -> &str {
        &self.fields[field.0]
    }

    /// The kind a field is named after, if any: it holds only that kind.
    pub(crate) fn named_kind(&self, field: Field) -> Option<Kind> {
        self.kind(self.field_name(field))
    }

    /// Every token kind, in order.
    pub(crate) fn all_tokens(&self) -> impl Iterator<Item = Token> {
        (0..self.tokens.variants.len()).map(Token)
    }

    /// Each node kind with a production, and the production.
    pub(crate) fn produced(&self) -> impl Iterator<Item = (Kind, &[Term])> {
        let variants = self.nodes.variants.iter().enumerate();
        variants.filter_map(|(i, v)| Some((Kind(i), v.production.as_deref()?)))
    }

    pub(crate) fn production(&self, kind: Kind) -> Option<&[Term]> {
        self.nodes.variants[kind.0].production.as_deref()
    }
}
