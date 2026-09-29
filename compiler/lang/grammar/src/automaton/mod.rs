//! Each production as an automaton over what it reads one by one: its token and word terms and
//! its children, each a position (Glushkov's construction). A set of positions is where a reading
//! of the production may stand; what may come next are the positions following them.

use std::sync::LazyLock;

use crate::{NodeKind, NodeTerm};

mod first;
mod lead;
mod read;
mod sets;

use read::Restriction;
pub use read::{Child, Read};
pub use sets::{KindSet, TokenSet};

/// Positions of one production, as bits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Positions(u64);

impl Positions {
    pub const EMPTY: Self = Self(0);

    #[must_use]
    pub const fn single(p: usize) -> Self {
        Self(1 << p)
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    #[must_use]
    pub const fn contains(self, p: usize) -> bool {
        self.0 & (1 << p) != 0
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    /// The positions, ascending.
    pub fn iter(self) -> impl Iterator<Item = usize> {
        (0..u64::BITS as usize).filter(move |&p| self.contains(p))
    }

    #[must_use]
    pub fn filter(self, keep: impl Fn(usize) -> bool) -> Self {
        self.iter().filter(|&p| keep(p)).fold(Self::EMPTY, |set, p| set.union(Self::single(p)))
    }
}

/// A production read position by position.
#[derive(Clone, Debug)]
pub struct Automaton {
    /// What each position reads.
    pub reads: Vec<Read>,
    /// The positions a reading may start at.
    pub first: Positions,
    /// The positions that may follow each position.
    pub follow: Vec<Positions>,
    /// The positions a reading may end at.
    pub last: Positions,
    /// Whether the production may read nothing.
    pub nullable: bool,
    /// The positions whose token anchors the node: token and word terms under no repetition.
    pub anchors: Positions,
}

/// What a term contributes: where it may start and end, and whether it may be empty.
#[derive(Clone, Copy)]
struct Part {
    first: Positions,
    last: Positions,
    nullable: bool,
}

const EMPTY: Part = Part { first: Positions::EMPTY, last: Positions::EMPTY, nullable: true };

impl Automaton {
    fn of(kind: NodeKind, terms: &[NodeTerm]) -> Self {
        let mut automaton = Self {
            reads: Vec::new(),
            first: Positions::EMPTY,
            follow: Vec::new(),
            last: Positions::EMPTY,
            nullable: true,
            anchors: Positions::EMPTY,
        };
        let whole = automaton.sequence(kind, terms, false);
        assert!(automaton.reads.len() <= u64::BITS as usize, "a production has at most 64 terms");
        automaton.first = whole.first;
        automaton.last = whole.last;
        automaton.nullable = whole.nullable;
        automaton
    }

    /// Where any position of `from` may lead, or the first positions when `from` is the start.
    #[must_use]
    pub fn next(&self, from: Option<Positions>) -> Positions {
        match from {
            None => self.first,
            Some(from) => from.iter().fold(Positions::EMPTY, |next, p| next.union(self.follow[p])),
        }
    }

    /// Whether a reading standing at `at` (the start when `None`) may end there.
    #[must_use]
    pub fn accepts(&self, at: Option<Positions>) -> bool {
        at.map_or(self.nullable, |at| at.intersects(self.last))
    }

    /// A position reading `read`; one reading a token under no repetition may anchor the node.
    fn leaf(&mut self, read: Read, repeated: bool) -> Part {
        let p = self.reads.len();
        if !repeated && !matches!(read, Read::Child(_)) {
            self.anchors = self.anchors.union(Positions::single(p));
        }
        self.reads.push(read);
        self.follow.push(Positions::EMPTY);
        Part { first: Positions::single(p), last: Positions::single(p), nullable: false }
    }

    /// The shortest way from `at` (the start when `None`) to the end: the positions read on it,
    /// in order; `None` where no way ends.
    #[must_use]
    pub fn way_out(&self, at: Option<Positions>) -> Option<Vec<usize>> {
        if self.accepts(at) {
            return Some(Vec::new());
        }
        // Breadth first over positions, each reached once, from those that may come next.
        let mut came_from: Vec<Option<Option<usize>>> = vec![None; self.reads.len()];
        let mut frontier: Vec<usize> = self.next(at).iter().collect();
        for &p in &frontier {
            came_from[p] = Some(None);
        }
        while !frontier.is_empty() {
            if let Some(&end) = frontier.iter().find(|&&p| self.last.contains(p)) {
                let mut way = vec![end];
                while let Some(Some(before)) = came_from[*way.last().unwrap_or(&end)] {
                    way.push(before);
                }
                way.reverse();
                return Some(way);
            }
            let mut next = Vec::new();
            for p in frontier {
                for q in self.follow[p].iter() {
                    if came_from[q].is_none() {
                        came_from[q] = Some(Some(p));
                        next.push(q);
                    }
                }
            }
            frontier = next;
        }
        None
    }

    fn links(&mut self, from: Positions, to: Positions) {
        for p in from.iter() {
            self.follow[p] = self.follow[p].union(to);
        }
    }

    fn sequence(&mut self, kind: NodeKind, terms: &[NodeTerm], repeated: bool) -> Part {
        terms.iter().fold(EMPTY, |before, term| {
            let after = self.term(kind, term, repeated);
            self.links(before.last, after.first);
            Part {
                first: if before.nullable { before.first.union(after.first) } else { before.first },
                last: if after.nullable { before.last.union(after.last) } else { after.last },
                nullable: before.nullable && after.nullable,
            }
        })
    }

    fn term(&mut self, kind: NodeKind, term: &NodeTerm, repeated: bool) -> Part {
        match *term {
            NodeTerm::Sequence(terms) => self.sequence(kind, terms, repeated),
            NodeTerm::Optional(terms) => Part { nullable: true, ..self.sequence(kind, terms, repeated) },
            NodeTerm::Many(terms) => {
                let once = self.sequence(kind, terms, true);
                self.links(once.last, once.first);
                Part { nullable: true, ..once }
            }
            NodeTerm::Choice(terms) => terms.iter().fold(Part { nullable: false, ..EMPTY }, |all, term| {
                let one = self.term(kind, term, repeated);
                Part {
                    first: all.first.union(one.first),
                    last: all.last.union(one.last),
                    nullable: all.nullable || one.nullable,
                }
            }),
            // `(item (separator item)* separator?)?`
            NodeTerm::Separated(item, separator) => {
                let (item, separator) = (self.term(kind, item, true), self.term(kind, separator, true));
                self.links(item.last, separator.first);
                self.links(separator.last, item.first);
                Part { first: item.first, last: item.last.union(separator.last), nullable: true }
            }
            NodeTerm::Token(kinds) => self.leaf(Read::Token(TokenSet::of(kinds)), repeated),
            NodeTerm::Word(words) => self.leaf(Read::word(words), repeated),
            NodeTerm::Field(field) => self.leaf(Read::child(kind, field, Restriction::None), repeated),
            NodeTerm::Of(field, kinds) => {
                self.leaf(Read::child(kind, field, Restriction::Of(kinds)), repeated)
            }
            NodeTerm::Starting(field, tokens) => {
                self.leaf(Read::child(kind, field, Restriction::Starting(tokens)), repeated)
            }
        }
    }
}

/// The automaton of each kind with a production, by the kind's index.
static AUTOMATA: LazyLock<Vec<Option<Automaton>>> = LazyLock::new(|| {
    NodeKind::ALL.iter().map(|&k| k.production().map(|terms| Automaton::of(k, terms))).collect()
});

impl NodeKind {
    /// Its index among [`NodeKind::ALL`].
    #[must_use]
    pub fn index(self) -> usize {
        self as usize
    }

    /// This kind's production as an automaton; `None` for a kind declared without one.
    #[must_use]
    pub fn automaton(self) -> Option<&'static Automaton> {
        AUTOMATA[self.index()].as_ref()
    }
}

#[cfg(test)]
mod tests;
