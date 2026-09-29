//! Whether a node spells its kind's production: its tokens and children, in source order,
//! matched against the terms as a regular expression, by the set of positions each term may end
//! at, so that no input makes the match backtrack exponentially or recurse per element.
//!
//! What recovery built fits as the parser leaves it: an absent token stands for the term naming
//! it, and an `Error` child for a term naming a child, or for skipped source matching none.

use crate::{NodeField, NodeKind, NodeTerm, TokenKind};

/// One element of a node as its production reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Element<'a> {
    /// A token of the node: its anchor or a `punct` child.
    Token { kind: TokenKind, text: &'a str },
    /// A child, by the field it fills and its kind, with the kind it ends with through
    /// transparent kinds and the token it starts with.
    Child { field: NodeField, kind: NodeKind, ends: NodeKind, first: TokenKind },
    /// A token recovery found absent.
    Absent,
}

/// Positions between elements, ascending, each once.
#[derive(Clone)]
struct Positions(Vec<usize>);

impl FromIterator<usize> for Positions {
    fn from_iter<I: IntoIterator<Item = usize>>(iter: I) -> Self {
        let mut positions: Vec<usize> = iter.into_iter().collect();
        positions.sort_unstable();
        positions.dedup();
        Self(positions)
    }
}

impl Positions {
    fn contains(&self, p: usize) -> bool {
        self.0.binary_search(&p).is_ok()
    }

    fn union(self, other: Self) -> Self {
        self.0.into_iter().chain(other.0).collect()
    }
}

impl NodeKind {
    /// Whether `elements` spell this kind's production; any do for a kind without one.
    #[must_use]
    pub fn spelled_by(self, elements: &[Element<'_>]) -> bool {
        let Some(terms) = self.production() else { return true };
        let matcher = Matcher { elements };
        matcher.skipped(matcher.sequence(terms, Positions(vec![0]))).contains(elements.len())
    }
}

impl Element<'_> {
    /// Whether this element is what the term `Token`, `Word`, `Field` or `Of` stands for.
    fn fits(self, term: NodeTerm) -> bool {
        match (term, self) {
            (NodeTerm::Token(_) | NodeTerm::Word(_), Self::Absent) => true,
            (NodeTerm::Token(kinds), Self::Token { kind, .. }) => kinds.contains(&kind),
            (NodeTerm::Word(words), Self::Token { kind, text }) => {
                (kind == TokenKind::Ident || kind.text().is_some()) && words.contains(&text)
            }
            (NodeTerm::Field(field), Self::Child { field: child, .. }) => field == child,
            // What recovery built at the end fits, as it fits any field: `.$` missing its name.
            (NodeTerm::Of(field, kinds), Self::Child { field: child, kind, ends, .. }) => {
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
            | NodeTerm::Starting(..) => self
                .skipped(starts)
                .0
                .into_iter()
                .filter(|&p| self.elements.get(p).is_some_and(|e| e.fits(term)))
                .map(|p| p + 1)
                .collect(),
            NodeTerm::Sequence(terms) => self.sequence(terms, starts),
            NodeTerm::Optional(terms) => {
                let taken = self.sequence(terms, starts.clone());
                starts.union(taken)
            }
            NodeTerm::Many(terms) => self.repeated(starts, |at| self.sequence(terms, at)),
            NodeTerm::Choice(terms) => terms.iter().flat_map(|&t| self.term(t, starts.clone()).0).collect(),
            // `(item (separator item)* separator?)?`
            NodeTerm::Separated(&item, &separator) => {
                let items = self.term(item, starts.clone());
                let items = self.repeated(items, |at| self.term(item, self.term(separator, at)));
                let trailing = self.term(separator, items.clone());
                starts.union(items).union(trailing)
            }
        }
    }

    /// `starts`, and the positions past any `Error` children after them.
    fn skipped(&self, starts: Positions) -> Positions {
        let error =
            |p: &usize| matches!(self.elements.get(*p), Some(Element::Child { kind: NodeKind::Error, .. }));
        if !starts.0.iter().any(error) {
            return starts;
        }
        self.repeated(starts, |at| at.0.into_iter().filter(error).map(|p| p + 1).collect())
    }

    /// Where any number of `step` may end, started at any of `starts`: only positions not reached
    /// yet step again, so the loop ends.
    fn repeated(&self, starts: Positions, step: impl Fn(Positions) -> Positions) -> Positions {
        let mut reached = vec![false; self.elements.len() + 1];
        let mut frontier = starts;
        frontier.0.iter().for_each(|&p| reached[p] = true);
        while !frontier.0.is_empty() {
            frontier = Positions(
                step(frontier).0.into_iter().filter(|&p| !std::mem::replace(&mut reached[p], true)).collect(),
            );
        }
        Positions(reached.iter().enumerate().filter_map(|(p, &r)| r.then_some(p)).collect())
    }
}
