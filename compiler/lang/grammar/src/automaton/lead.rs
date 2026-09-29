//! Productions beginning with an operand, which continue one read before them, and the tokens
//! the others start with alone.

use std::sync::LazyLock;

use crate::{NodeField, NodeKind};

use super::{Automaton, Child, KindSet, Positions, Read, TokenSet};

impl Child {
    /// Whether this child may be an operand: any, or one of the kinds it names.
    #[must_use]
    pub fn takes_operands(&self) -> bool {
        self.kinds.is_none_or(|kinds| !kinds.intersection(NodeKind::operands()).is_empty())
    }

    /// Whether the kinds this child names may be those of nodes transparent ones end with.
    #[must_use]
    pub fn through(&self) -> bool {
        self.kinds.is_some() && !self.exact
    }

    /// Whether a node of `kind` may stand here as it is, besides the kinds `context` makes
    /// operands.
    #[must_use]
    pub fn holds(&self, kind: NodeKind, context: KindSet) -> bool {
        let held = match self.kinds {
            Some(kinds) => kinds.contains(kind),
            None => NodeKind::operands().contains(kind) || self.admitted.contains(kind),
        };
        held || context.contains(kind) || kind == NodeKind::Error
    }
}

impl Automaton {
    /// The first positions reading an operand: where a reading continues one read before it.
    #[must_use]
    pub fn leading(&self) -> Positions {
        self.first.filter(|p| matches!(&self.reads[p], Read::Child(c) if c.takes_operands()))
    }
}

/// For each field, the parts that may lead to what it admits: those it admits, and those the
/// leading operand of one of them may be, and so on (`n := p` leads to `n := p = v` in a head).
static LEADING: LazyLock<Vec<KindSet>> = LazyLock::new(|| {
    let admitted = |field: &NodeField| {
        let rows = crate::ADMITS.iter().filter(|row| row.fields.contains(field));
        rows.fold(KindSet::EMPTY, |set, row| set.union(KindSet::of(row.kinds))).leading()
    };
    NodeField::ALL.iter().map(admitted).collect()
});

impl NodeField {
    /// The parts that may lead to one this field admits, those included.
    #[must_use]
    pub fn leading(self) -> KindSet {
        LEADING[self as usize]
    }
}

impl NodeKind {
    /// Whether a node of this kind may continue an operand read before it.
    #[must_use]
    pub fn continues(self) -> bool {
        self.automaton().is_some_and(|a| !a.leading().is_empty())
    }

    /// The tokens a node of this kind may start with where no operand was read before it: those
    /// of its first tokens and of first children that are no operands (`mut` in `mut x := 1`).
    #[must_use]
    pub fn starts_alone(self) -> TokenSet {
        self.automaton().map_or(TokenSet::EMPTY, |a| a.first_of(a.first.filter(|p| !a.leading().contains(p))))
    }
}

impl KindSet {
    /// These kinds, and the parts that may lead to one of them, and so on.
    #[must_use]
    pub fn leading(self) -> Self {
        let mut set = self;
        loop {
            let wider = set.iter().fold(set, |all, k| all.union(leads(k)));
            if wider == set {
                return set;
            }
            set = wider;
        }
    }
}

/// The kinds the leading operand of `kind` may be, besides operands.
fn leads(kind: NodeKind) -> KindSet {
    let Some(automaton) = kind.automaton() else { return KindSet::EMPTY };
    automaton.leading().iter().fold(KindSet::EMPTY, |set, p| match &automaton.reads[p] {
        Read::Child(child) => set.union(child.admitted).union(child.kinds.unwrap_or(KindSet::EMPTY)),
        _ => set,
    })
}
