//! The frames of the stack the engine reads with: from the root's, readings of nodes alternate
//! with the operands they read.

use vun_grammar::{Automaton, KindSet, Level, NodeKind, Positions};

use super::spec::Spec;
use super::start::BY_RANK;
use super::tree::Piece;
use crate::cst::Anchor;

pub(in crate::parser) enum Frame {
    Reading(Reading),
    Operand(Operand),
}

/// A kind a node being read may be, and the positions of its production it stands at; `None`
/// before the first.
#[derive(Clone, Copy, Debug)]
pub(super) struct Candidate {
    pub(super) kind: NodeKind,
    pub(super) at: Option<Positions>,
}

impl Candidate {
    pub(super) fn automaton(self) -> &'static Automaton {
        self.kind.automaton().expect("candidates have productions")
    }

    pub(super) fn next(self) -> Positions {
        self.automaton().next(self.at)
    }

    pub(super) fn accepts(self) -> bool {
        self.automaton().accepts(self.at)
    }
}

/// A node being read: the kinds it may be move together, each at its positions, and it is
/// written as one of those that may end where it ends.
pub(in crate::parser) struct Reading {
    pub(super) candidates: Vec<Candidate>,
    /// The first token an anchoring position read, or supposed absent.
    pub(super) anchor: Option<Anchor>,
    pub(super) pieces: Vec<Piece>,
    /// The candidates after each move, and where each move's end, for the path the kind written
    /// took.
    trail: Vec<Candidate>,
    steps: Vec<Step>,
    /// Whether it chains an operator of a level without grouping, which makes it an `Error`.
    pub(super) wrong: bool,
    /// Where recovery last supposed something absent in it, so that it does once per token.
    pub(super) supposed: Option<usize>,
}

/// A move of a reading: where its candidates end in the trail, and whether it read a child.
struct Step {
    end: usize,
    child: bool,
}

impl Reading {
    /// A reading of one of `kinds`, preferred first, before its first token.
    pub(super) fn new(kinds: KindSet) -> Self {
        let candidates: Vec<Candidate> = BY_RANK
            .iter()
            .filter(|&&k| kinds.contains(k))
            .map(|&kind| Candidate { kind, at: None })
            .collect();
        let (trail, steps, pieces) = (Vec::new(), Vec::new(), Vec::new());
        Self { candidates, anchor: None, pieces, trail, steps, wrong: false, supposed: None }
    }

    /// Records where the candidates stand after a move, which read a child or not.
    pub(super) fn moved(&mut self, child: bool) {
        self.trail.extend_from_slice(&self.candidates);
        self.steps.push(Step { end: self.trail.len(), child });
    }

    /// Moves each candidate to the positions `at` gives it, in order; those given none drop.
    pub(super) fn move_to(&mut self, at: impl IntoIterator<Item = Positions>) {
        let moved = self.candidates.iter().zip(at).map(|(&c, at)| Candidate { at: Some(at), ..c });
        self.candidates = moved.filter(|c| c.at.is_some_and(|at| !at.is_empty())).collect();
    }

    /// The kinds of the candidates that may end, standing where `at` says.
    pub(super) fn ending(&self, at: impl Fn(Candidate) -> Option<Positions>) -> KindSet {
        let ending = self.candidates.iter().filter(|&&c| c.automaton().accepts(at(c)));
        ending.fold(KindSet::EMPTY, |set, c| set.with(c.kind))
    }

    /// The positions `kind` stood at after each move, the last first, with whether the move read
    /// a child.
    pub(super) fn path(&self, kind: NodeKind) -> impl Iterator<Item = (Positions, bool)> {
        self.steps.iter().enumerate().rev().map(move |(i, step)| {
            let start = i.checked_sub(1).map_or(0, |before| self.steps[before].end);
            let at = self.trail[start..step.end].iter().find(|c| c.kind == kind).and_then(|c| c.at);
            (at.unwrap_or_default(), step.child)
        })
    }
}

/// A child being read for the positions of the reading below it, which stand at them.
pub(in crate::parser) struct Operand {
    /// What those positions take, merged.
    pub(super) spec: Spec,
    /// The node read, once complete; while a reading above it goes on, `None`.
    pub(super) piece: Option<u32>,
    /// How many forms continued it, each a level of nesting.
    pub(super) links: u32,
    /// The level of the operator without grouping it was last continued with.
    pub(super) chained: Option<Level>,
}

/// What an operand holds, or would once the frames above it end: a node, or the kinds the
/// reading above it may end as, and those of the nodes it would end with through transparent ones.
#[derive(Clone, Copy)]
pub(super) enum Content {
    Node(u32),
    Kinds { kinds: KindSet, ends: KindSet },
}
