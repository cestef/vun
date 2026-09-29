//! Productions resolved: terms naming tokens and fields by id, the fields a production has and how
//! many nodes each holds, and the sequences its choices spell.

use super::{Field, Kind, Model, Token};

/// A term of a production, resolved.
pub(crate) enum Term {
    /// A token of one of these kinds.
    Tokens(Vec<Token>),
    /// A name, or a token spelled, as one of these words.
    Words(Vec<String>),
    /// A child in this field, restricted by a table if one is named.
    Field(Field, Option<Restriction>),
    Optional(Vec<Term>),
    Many(Vec<Term>),
    Separated(Box<Term>, Box<Term>),
    Choice(Vec<Term>),
    Sequence(Vec<Term>),
}

/// What a table allows of a child: its kind, the kind it ends with through transparent kinds, or
/// the token it starts with.
pub(crate) enum Restriction {
    Of(Table<Kind>),
    Ending(Table<Kind>),
    Starting(Table<Token>),
}

/// A table a restriction names: its name, which generated code refers to it by, and what it lists.
pub(crate) struct Table<I> {
    pub(crate) name: String,
    pub(crate) members: Vec<I>,
}

/// How many nodes a field holds, as the variants of `Count` in `nodes.vun` name it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Count {
    One,
    Optional,
    Many,
}

impl Count {
    pub(crate) const ALL: [Self; 3] = [Self::One, Self::Optional, Self::Many];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::One => "one",
            Self::Optional => "optional",
            Self::Many => "many",
        }
    }

    /// Standing inside a term that makes each field it holds `inner`, unless already `many`.
    fn within(self, inner: Self) -> Self {
        if self == Self::Many { self } else { inner }
    }
}

/// One child position of a kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Slot {
    pub(crate) field: Field,
    pub(crate) count: Count,
}

impl Term {
    /// The fields of `terms` in the order they stand, each with its count: `many` in `many` or
    /// `separated`, else `optional` in `optional` or `choice`, else `one`. A field standing in
    /// several places in a row holds `many`; `None` for one standing again after another field,
    /// which could not be told apart from it.
    pub(crate) fn slots(terms: &[Self]) -> Result<Vec<Slot>, Field> {
        let mut standings = Vec::new();
        for t in terms {
            t.fields(Count::One, &mut standings);
        }
        let mut slots: Vec<Slot> = Vec::new();
        for standing in standings {
            let again = slots.last().is_some_and(|last| last.field == standing.field);
            if again {
                slots.last_mut().expect("the slot standing again").count = Count::Many;
            } else if slots.iter().any(|s| s.field == standing.field) {
                return Err(standing.field);
            } else {
                slots.push(standing);
            }
        }
        Ok(slots)
    }

    fn fields(&self, count: Count, into: &mut Vec<Slot>) {
        let (terms, count): (Vec<&Self>, _) = match self {
            Self::Field(field, _) => return into.push(Slot { field: *field, count }),
            Self::Tokens(_) | Self::Words(_) => return,
            Self::Sequence(terms) => (terms.iter().collect(), count),
            Self::Optional(terms) | Self::Choice(terms) => {
                (terms.iter().collect(), count.within(Count::Optional))
            }
            Self::Many(terms) => (terms.iter().collect(), Count::Many),
            Self::Separated(item, separator) => (vec![item, separator], Count::Many),
        };
        for t in terms {
            t.fields(count, into);
        }
    }

    /// The sequences of terms `terms` spell, choices and sequences spelled out: `a (b | c)` is
    /// `a b` and `a c`. Repetitions and options stay terms.
    pub(crate) fn spelled_out(terms: &[Self]) -> Vec<Vec<&Self>> {
        terms.iter().fold(vec![Vec::new()], |sequences, term| {
            let tails: Vec<Vec<&Self>> = match term {
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

    /// Every token and word term, in order.
    pub(crate) fn leaves<'t>(&'t self, into: &mut Vec<&'t Self>) {
        match self {
            Self::Tokens(_) | Self::Words(_) => into.push(self),
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

    /// The leaves of every term of `terms`, in order.
    pub(crate) fn all_leaves(terms: &[Self]) -> Vec<&Self> {
        let mut leaves = Vec::new();
        for t in terms {
            t.leaves(&mut leaves);
        }
        leaves
    }
}

impl Model {
    /// The slots of the production of `kind`; empty for a kind without one.
    pub(crate) fn slots(&self, kind: Kind) -> &[Slot] {
        &self.slots[kind.0]
    }
}
