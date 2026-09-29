//! Whether a node spells its kind's production: its tokens and children, in source order,
//! matched against the terms as a regular expression, by the set of positions each term may end
//! at, so that no input makes the match backtrack exponentially or recurse per element.

use std::collections::BTreeSet;

use crate::{NodeField, NodeKind, NodeTerm, TokenKind};

/// One element of a node as its production reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Element<'a> {
    /// A token of the node: its anchor or a `punct` child.
    Token { kind: TokenKind, text: &'a str },
    /// A child, by the field it fills and its kind, with the kind it ends with through
    /// transparent kinds and the token it starts with.
    Child { field: NodeField, kind: NodeKind, ends: NodeKind, first: TokenKind },
}

/// Positions between elements.
type Positions = BTreeSet<usize>;

impl NodeKind {
    /// Whether `elements` spell this kind's production; any do for a kind without one.
    #[must_use]
    pub fn spelled_by(self, elements: &[Element<'_>]) -> bool {
        let Some(terms) = self.production() else { return true };
        Matcher { elements }.sequence(terms, Positions::from([0])).contains(&elements.len())
    }
}

impl Element<'_> {
    /// Whether this element is what the term `Token`, `Word`, `Field` or `Of` stands for.
    fn fits(self, term: NodeTerm) -> bool {
        match (term, self) {
            (NodeTerm::Token(kinds), Self::Token { kind, .. }) => kinds.contains(&kind),
            (NodeTerm::Word(words), Self::Token { kind, text }) => {
                (kind == TokenKind::Ident || kind.text().is_some()) && words.contains(&text)
            }
            (NodeTerm::Field(field), Self::Child { field: child, .. }) => field == child,
            (NodeTerm::Of(field, kinds), Self::Child { field: child, kind, .. }) => {
                field == child && kinds.contains(&kind)
            }
            // What recovery built at the end fits, as it fits any field: `.$` missing its name.
            (NodeTerm::Ending(field, kinds), Self::Child { field: child, kind, ends, .. }) => {
                field == child && (kinds.contains(&kind) || kinds.contains(&ends) || ends == NodeKind::Error)
            }
            (NodeTerm::Starting(field, tokens), Self::Child { field: child, first, .. }) => {
                field == child && tokens.contains(&first)
            }
            _ => false,
        }
    }
}

struct Matcher<'e, 'a> {
    elements: &'e [Element<'a>],
}

impl Matcher<'_, '_> {
    /// Where `terms` in sequence may end, started at any of `starts`.
    fn sequence(&self, terms: &[NodeTerm], starts: Positions) -> Positions {
        terms.iter().fold(starts, |at, &t| self.term(t, at))
    }

    fn term(&self, term: NodeTerm, starts: Positions) -> Positions {
        match term {
            NodeTerm::Token(_)
            | NodeTerm::Word(_)
            | NodeTerm::Field(_)
            | NodeTerm::Of(..)
            | NodeTerm::Ending(..)
            | NodeTerm::Starting(..) => starts
                .into_iter()
                .filter(|&p| self.elements.get(p).is_some_and(|e| e.fits(term)))
                .map(|p| p + 1)
                .collect(),
            NodeTerm::Sequence(terms) => self.sequence(terms, starts),
            NodeTerm::Optional(terms) => {
                let taken = self.sequence(terms, starts.clone());
                starts.union(&taken).copied().collect()
            }
            NodeTerm::Many(terms) => Self::repeated(starts, |at| self.sequence(terms, at)),
            NodeTerm::Choice(terms) => terms.iter().flat_map(|&t| self.term(t, starts.clone())).collect(),
            // `(item (separator item)* separator?)?`
            NodeTerm::Separated(&item, &separator) => {
                let items = self.term(item, starts.clone());
                let items = Self::repeated(items, |at| self.term(item, self.term(separator, at)));
                let trailing = self.term(separator, items.clone());
                starts.into_iter().chain(items).chain(trailing).collect()
            }
        }
    }

    /// Where any number of `step` may end, started at any of `starts`: only positions not reached
    /// yet step again, so the loop ends.
    fn repeated(starts: Positions, step: impl Fn(Positions) -> Positions) -> Positions {
        let mut reached = starts.clone();
        let mut frontier = starts;
        while !frontier.is_empty() {
            frontier = step(frontier).into_iter().filter(|p| !reached.contains(p)).collect();
            reached.extend(&frontier);
        }
        reached
    }
}
