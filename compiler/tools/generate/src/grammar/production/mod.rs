//! Productions: what a kind's entry `.name(terms...)` spells, read by shape (the forms are in the
//! doc of `NodeKind` in `nodes.vun`), the fields and counts derived from it, and its tokens
//! resolved, for every consumer: the Rust tables and the editor grammars.

use std::borrow::Cow;

use vun_diag::Msg;
use vun_syntax::literal::Literal;

use super::{GENERATOR, SHAPE, Slot};
use crate::generate::Read;

/// The tags of the compound terms.
const TOKEN: &str = "token";
const WORD: &str = "word";
const OPTIONAL: &str = "optional";
const MANY: &str = "many";
const SEPARATED: &str = "separated";
const CHOICE: &str = "choice";
const SEQ: &str = "seq";
const OF: &str = "of";
const ENDING: &str = "ending";
const STARTING: &str = "starting";

/// The count of a field outside any compound term; the others are the tags above.
const ONE: &str = "one";

mod tokens;

pub(in crate::grammar) use tokens::Tokens;

/// A term of a production, whose tokens are `L`: as written, or resolved.
pub(super) enum Term<'s, L> {
    Leaf(L),
    /// A child in this field, of a kind a table lists if one is named.
    Field(&'s str, Option<Restriction<'s>>),
    Optional(Vec<Self>),
    Many(Vec<Self>),
    Separated(Box<Self>, Box<Self>),
    Choice(Vec<Self>),
    Sequence(Vec<Self>),
}

/// What a table allows of a child: its kind, the kind it ends with through transparent kinds, or
/// the token it starts with.
#[derive(Clone, Copy)]
pub(super) enum Restriction<'s> {
    Of(&'s str),
    Ending(&'s str),
    Starting(&'s str),
}

/// A token as written.
pub(super) enum Written<'s> {
    /// A token or word by its spelling.
    Text(Cow<'s, str>),
    /// A token of these kinds, lists of the token kind, or tables of tokens.
    Token(Vec<&'s str>),
    /// A name or keyword spelled as a variant of this kind, or of this list of a kind.
    Word(&'s str),
}

/// A token resolved.
pub(super) enum Resolved<'s> {
    /// A token of one of these kinds, by their names.
    Tokens(Vec<&'s str>),
    /// A name, or a token spelled, as one of these words.
    Words(Vec<Cow<'s, str>>),
}

impl<'s> Term<'s, Written<'s>> {
    pub(super) fn read(value: &Literal<'s>) -> Option<Self> {
        let all = |values: &[Literal<'s>]| values.iter().map(Self::read).collect::<Option<Vec<_>>>();
        let names = |values: &[Literal<'s>]| values.iter().map(Literal::as_name).collect::<Option<Vec<_>>>();
        Some(match value {
            Literal::String(text) => Self::Leaf(Written::Text(text.clone())),
            Literal::Name(field) => Self::Field(field, None),
            Literal::Tagged(tag, payload) => match (*tag, payload.as_slice()) {
                (TOKEN, kinds) if !kinds.is_empty() => Self::Leaf(Written::Token(names(kinds)?)),
                (WORD, [Literal::Name(kind)]) => Self::Leaf(Written::Word(kind)),
                (OPTIONAL, terms) => Self::Optional(all(terms)?),
                (MANY, terms) => Self::Many(all(terms)?),
                (SEPARATED, [item, separator]) => {
                    Self::Separated(Box::new(Self::read(item)?), Box::new(Self::read(separator)?))
                }
                (CHOICE, terms) => Self::Choice(all(terms)?),
                (SEQ, terms) => Self::Sequence(all(terms)?),
                (OF, [Literal::Name(field), Literal::Name(table)]) => {
                    Self::Field(field, Some(Restriction::Of(table)))
                }
                (ENDING, [Literal::Name(field), Literal::Name(table)]) => {
                    Self::Field(field, Some(Restriction::Ending(table)))
                }
                (STARTING, [Literal::Name(field), Literal::Name(table)]) => {
                    Self::Field(field, Some(Restriction::Starting(table)))
                }
                _ => return None,
            },
            _ => return None,
        })
    }
}

impl<'s, L> Term<'s, L> {
    /// The fields of `terms`, in the order they stand, each with its count: `many` in `many` or
    /// `separated`, else `optional` in `optional` or `choice`, else `one`. A field standing in
    /// several places in a row holds `many`; standing again after another field, it could not be
    /// told apart from it.
    pub(super) fn slots(owner: &str, terms: &[Self]) -> Read<Vec<Slot<'s>>> {
        let mut standings: Vec<Slot<'s>> = Vec::new();
        for t in terms {
            t.fields(ONE, &mut standings);
        }
        let mut slots: Vec<Slot<'s>> = Vec::new();
        for standing in standings {
            let again = slots.last().is_some_and(|last| last.field == standing.field);
            if again {
                slots.last_mut().expect("the slot standing again").count = MANY;
            } else if slots.iter().any(|s| s.field == standing.field) {
                let msg = Msg::new().text("the field ").code(standing.field).text(" stands apart twice");
                return Err(GENERATOR.error(SHAPE, owner, msg));
            } else {
                slots.push(standing);
            }
        }
        Ok(slots)
    }

    fn fields(&self, count: &'s str, into: &mut Vec<Slot<'s>>) {
        let looser = |tag: &'s str| if count == MANY { MANY } else { tag };
        let (terms, count): (Vec<&Self>, _) = match self {
            Self::Field(field, _) => return into.push(Slot { count, field }),
            Self::Leaf(_) => return,
            Self::Sequence(terms) => (terms.iter().collect(), count),
            Self::Optional(terms) | Self::Choice(terms) => (terms.iter().collect(), looser(OPTIONAL)),
            Self::Many(terms) => (terms.iter().collect(), MANY),
            Self::Separated(item, separator) => (vec![item, separator], MANY),
        };
        for t in terms {
            t.fields(count, into);
        }
    }

    /// This term with each leaf mapped by `leaf`.
    fn map<M>(&self, leaf: &mut impl FnMut(&L) -> Read<M>) -> Read<Term<'s, M>> {
        let mut all = |terms: &[Self]| terms.iter().map(|t| t.map(leaf)).collect::<Read<Vec<_>>>();
        Ok(match self {
            Self::Leaf(l) => Term::Leaf(leaf(l)?),
            Self::Field(field, of) => Term::Field(field, *of),
            Self::Optional(terms) => Term::Optional(all(terms)?),
            Self::Many(terms) => Term::Many(all(terms)?),
            Self::Choice(terms) => Term::Choice(all(terms)?),
            Self::Sequence(terms) => Term::Sequence(all(terms)?),
            Self::Separated(item, separator) => {
                Term::Separated(Box::new(item.map(leaf)?), Box::new(separator.map(leaf)?))
            }
        })
    }

    /// The sequences of terms `terms` spell, choices and sequences spelled out: `a (b | c)` is
    /// `a b` and `a c`. Repetitions and options stay terms.
    pub(super) fn spelled_out<'t>(terms: &'t [Self]) -> Vec<Vec<&'t Self>> {
        terms.iter().fold(vec![Vec::new()], |sequences, term| {
            let tails: Vec<Vec<&'t Self>> = match term {
                Self::Sequence(inner) => Self::spelled_out(inner),
                Self::Choice(alternatives) => {
                    alternatives.iter().flat_map(|a| Self::spelled_out(std::slice::from_ref(a))).collect()
                }
                other => vec![vec![other]],
            };
            sequences
                .iter()
                .flat_map(|head| tails.iter().map(move |tail| head.iter().chain(tail).copied().collect()))
                .collect()
        })
    }

    /// Every leaf, in order.
    pub(super) fn leaves<'t>(&'t self, into: &mut Vec<&'t L>) {
        match self {
            Self::Leaf(l) => into.push(l),
            Self::Field(..) => {}
            Self::Optional(terms) | Self::Many(terms) | Self::Choice(terms) | Self::Sequence(terms) => {
                for t in terms {
                    t.leaves(into);
                }
            }
            Self::Separated(item, separator) => {
                item.leaves(into);
                separator.leaves(into);
            }
        }
    }
}
