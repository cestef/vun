//! Where a node starts: what a position takes, the kinds that may start there at a token, and
//! the forms continuing an operand read before them.

use std::sync::LazyLock;

use vun_grammar::{KindSet, Level, NodeKind, Positions, Read, TRANSPARENT, TokenKind};

use super::frame::{Content, Operand, Reading};
use super::spec::Spec;
use crate::cst::Anchor;
use crate::parser::Parser;

/// The kinds with productions, preferred first.
pub(super) static BY_RANK: LazyLock<Vec<NodeKind>> = LazyLock::new(|| {
    let mut kinds: Vec<NodeKind> =
        NodeKind::ALL.iter().copied().filter(|k| k.automaton().is_some()).collect();
    kinds.sort_by_key(|k| k.rank());
    kinds
});

/// The kinds continuing an operand, preferred first.
static CONTINUING: LazyLock<Vec<NodeKind>> =
    LazyLock::new(|| BY_RANK.iter().copied().filter(|k| k.continues()).collect());

/// The kinds that may start alone with each token, by the token's index.
static STARTING: LazyLock<Vec<KindSet>> = LazyLock::new(|| {
    let starting = |&token: &TokenKind| {
        let kinds = NodeKind::ALL.iter().filter(|k| k.starts_alone().contains(token));
        kinds.fold(KindSet::EMPTY, |set, &k| set.with(k))
    };
    TokenKind::ALL.iter().map(starting).collect()
});

/// The transparent kinds written so with each token, by the token's index.
static TRANSPARENT_WITH: LazyLock<Vec<KindSet>> = LazyLock::new(|| {
    let with = |&token: &TokenKind| {
        let rows = TRANSPARENT.iter().filter(|row| row.tokens.contains(&token));
        rows.fold(KindSet::EMPTY, |set, row| set.with(row.kind))
    };
    TokenKind::ALL.iter().map(with).collect()
});

impl Parser<'_, '_, '_> {
    /// Whether `read` reads the token `ahead` of the current one: a token of its kinds, or a name
    /// or keyword spelled as one of its words.
    pub(super) fn reads_token(&self, read: &Read, ahead: usize) -> bool {
        match read {
            Read::Token(set) => set.contains(self.nth(ahead)),
            Read::Word { words, tokens } => {
                tokens.contains(self.nth(ahead)) && words.contains(&self.text(self.current_plus(ahead)))
            }
            Read::Child(_) => false,
        }
    }

    /// Whether a node of `kind` may start at the token `ahead` with no operand before it: its
    /// production reads the token first, or a first child that is no operand starts with it.
    fn starts_here(&self, kind: NodeKind, ahead: usize) -> bool {
        let Some(automaton) = kind.automaton() else { return false };
        let firsts = automaton.first.filter(|p| !automaton.leading().contains(p));
        firsts.iter().any(|p| match &automaton.reads[p] {
            Read::Child(child) => {
                child.kinds.is_some_and(|kinds| kinds.iter().any(|k| self.starts_here(k, ahead)))
            }
            read => self.reads_token(read, ahead),
        })
    }

    /// The kinds a node read where `spec` says may be, starting at the token `ahead`: those
    /// reachable, and the transparent kinds standing for one named where written so.
    pub(super) fn alone(&self, spec: &Spec, ahead: usize) -> KindSet {
        let token = self.nth(ahead);
        if spec.starting.is_some_and(|s| !s.contains(token)) {
            return KindSet::EMPTY;
        }
        let starting = |set: KindSet| {
            let set = set.intersection(STARTING[token as usize]);
            set.iter().filter(|&k| self.starts_here(k, ahead)).fold(KindSet::EMPTY, KindSet::with)
        };
        starting(spec.reachable().union(spec.standing_for().intersection(TRANSPARENT_WITH[token as usize])))
    }

    /// Whether a child where `spec` says may start at the token `ahead`; a token the lexer
    /// reported stands for any operand.
    pub(super) fn starts(&self, spec: &Spec, ahead: usize) -> bool {
        (self.nth(ahead) == TokenKind::Error && spec.takes_operands()) || !self.alone(spec, ahead).is_empty()
    }

    /// What position `p` of a reading of `kind` takes: its child at its level, and for the last
    /// child of a transparent node, what the operand holding the node takes, `outer`, too.
    pub(super) fn taking(
        &self,
        reading: &Reading,
        kind: NodeKind,
        p: usize,
        outer: Option<&Spec>,
    ) -> Option<Spec> {
        let automaton = kind.automaton()?;
        let Read::Child(child) = &automaton.reads[p] else { return None };
        let edge = automaton.last.contains(p);
        let level = if edge { self.edge_level(kind, reading.anchor) } else { Level::LOWEST };
        match outer {
            Some(outer) if edge && self.transparent(kind, reading.anchor) => {
                let outer = outer.at(level);
                // Where the node may stand only as what it ends with, the child takes that alone.
                let own = Spec::child(child, level);
                Some(if outer.standing_for().contains(kind) { outer } else { own.union(outer) })
            }
            _ => Some(Spec::child(child, level)),
        }
    }

    /// What the positions `at` of a reading of `kind` take, merged.
    pub(super) fn taking_at(
        &self,
        reading: &Reading,
        kind: NodeKind,
        at: Positions,
        outer: Option<&Spec>,
    ) -> Option<Spec> {
        at.iter().filter_map(|p| self.taking(reading, kind, p, outer)).reduce(Spec::union)
    }

    /// How tightly a node of `kind` anchored at `anchor` holds the child at its end: as its level
    /// says, and for the infix form, tighter than its operator, whose level groups left.
    fn edge_level(&self, kind: NodeKind, anchor: Option<Anchor>) -> Level {
        let operator = anchor.map(|a| self.kind(a.token()).infix());
        match operator {
            Some(Some(op)) if kind == NodeKind::INFIX_FORM => op.level.tighter(),
            _ => kind.operand_level(),
        }
    }

    /// Whether a node of `kind` anchored at `anchor` is transparent.
    pub(super) fn transparent(&self, kind: NodeKind, anchor: Option<Anchor>) -> bool {
        let Some(Anchor::Token(t)) = anchor else { return false };
        TRANSPARENT_WITH[self.kind(t) as usize].contains(kind)
    }

    /// The level of a form of `kind` continuing at the current token: the operator's for the
    /// infix form.
    fn level(&self, kind: NodeKind) -> Option<Level> {
        if kind == NodeKind::INFIX_FORM { self.peek().infix().map(|op| op.level) } else { kind.level() }
    }

    /// The forms continuing `o`, holding `content`, at the current token: of those it may reach at
    /// its level or tighter, whose leading positions may hold what it holds, those reading the
    /// longest run of tokens.
    pub(super) fn continuation(&self, o: &Operand, content: Content) -> Option<KindSet> {
        if self.at(TokenKind::Eof) {
            return None;
        }
        let reachable = o.spec.reachable();
        let mut best = (0, KindSet::EMPTY);
        for &kind in CONTINUING.iter() {
            if !reachable.contains(kind) || self.level(kind).is_some_and(|l| l < o.spec.level) {
                continue;
            }
            let Some(automaton) = kind.automaton() else { continue };
            let held = self.leading(kind, content);
            let Some(run) = (!held.is_empty()).then(|| self.run(automaton, held)).flatten() else { continue };
            if run > best.0 {
                best = (run, KindSet::EMPTY);
            }
            if run == best.0 {
                best.1 = best.1.with(kind);
            }
        }
        (!best.1.is_empty()).then_some(best.1)
    }

    /// The leading positions of the production of `kind` that may hold `content`.
    pub(super) fn leading(&self, kind: NodeKind, content: Content) -> Positions {
        let Some(automaton) = kind.automaton() else { return Positions::EMPTY };
        let holds = |p: usize| match &automaton.reads[p] {
            Read::Child(child) => self.held(content, &Spec::child(child, Level::LOWEST)),
            _ => false,
        };
        automaton.leading().filter(holds)
    }

    /// Whether `content` may stand where `spec` says: a node as one of the kinds it spells, the
    /// kinds of a reading as one of them.
    pub(super) fn held(&self, content: Content, spec: &Spec) -> bool {
        match content {
            Content::Node(n) => self.tree.fits(n, spec, self.tokens),
            Content::Kinds { kinds, ends } => spec.may_hold(kinds, ends),
        }
    }
}
